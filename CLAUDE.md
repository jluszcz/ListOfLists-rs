# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build and Test

- `cargo build` - Build the project
- `cargo fmt` - Format the source code
- `cargo test` - Run all tests
- `cargo check` - Check for compilation errors without building
- `cargo clippy -- -D warnings` - Run Rust linter for code quality checks

### Running the Application

- `cargo run --bin main -- --help` - Show CLI help for the main generator
- `cargo run --bin lambda` - Run the Lambda function locally (requires AWS environment setup)

### Environment Variables

The application uses these environment variables:

- `LOL_GENERATOR_BUCKET` - Generator bucket name (used by Lambda; CLI defaults to `generator` for local use)
- `LOL_SITE_URL` - Site URL (e.g., "list-of-l.ist") (CLI only)

## Architecture

ListOfLists is a Rust application that generates static websites from JSON data, deployable to AWS S3. The application
has two main execution modes:

### Core Components

**Data Model (`lib.rs`)**

- `ListOfLists` - Main structure containing title, lists, and footer configuration
- `List` - Individual list with title, items, and visibility settings
- `ListItem` - Can be simple strings or items with tooltips
- Built-in validation for duplicate detection and data integrity

**Generator (`generator.rs`)**

- Template-based HTML generation using Minijinja templating engine
- Dual I/O abstraction supporting both local files and S3 operations
- HTML minification support for production deployments
- Custom Jinja filter `div_id_safe` for generating CSS-safe div IDs

**Template (`index.template`)**

- Minijinja template that renders the full HTML page for a site
- Receives `title`, `lists`, `site_url`, and optional `description`/`footer`/`footer_links` variables from the generator
- Iterates over non-hidden lists to build Bootstrap tab navigation and list content
- Supports two item types: plain strings and tooltip objects (`{ item, tooltip }`)
- When running locally, the generated `index.html` is written to `buckets/{site_url}/index.html` (e.g., `buckets/list-of-l.ist/index.html`)

**Executables**

- `main.rs` - CLI tool for local development and manual site generation
- `lambda.rs` - AWS Lambda function for automatic S3-triggered updates

### Data Flow

1. JSON configuration is read from S3 bucket or local file
2. HTML template (`index.template`) is loaded and processed
3. Data is validated and rendered through Minijinja templates
4. Generated HTML is optionally minified and written to destination

### S3 Integration

- Generator bucket: account/region-namespaced (e.g., `list-of-lists-{account_id}-{region}-an`) — stores JSON and templates for all sites
- Site bucket: `{site_url}` (hosts the generated static site)
- Lambda function triggers on S3 object changes for automatic updates

### Terraform Infrastructure

Terraform is split into independent root modules, each with its own S3 backend state (bucket `jluszcz-tf-state`,
region `us-east-2`):

- `shared/` — account-wide resources (generator S3 bucket, Lambda function + log group, shared IAM roles/policies,
  GitHub OIDC roles). Backend key `list-of-lists/shared`.
- `site-module/` — reusable module defining one site's resources (site S3 bucket, ACM cert, Route53 zone/records,
  CloudFront distribution + OAC, per-site GitHub update role). No backend/provider blocks of its own.
- `sites/<site_url>/` — one tiny root module per site. Each pins its own backend key
  (`list-of-lists/sites/<site_url>`) and providers, then calls `../../site-module`. **These directories are
  gitignored** so individual site identities never enter the repo; scaffold them locally with
  `scripts/new-site.sh <site_url> <github_org> <github_repo> [site_name]`.

Each directory carries a `.envrc` (direnv, also gitignored); `cd` into a dir to load its context. Run `terraform`
from within the target directory — no `-backend-config` injection or workspace switching needed. New machines:
`direnv allow` once per directory.
