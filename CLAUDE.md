# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build and Test

- `cargo build` - Build the project
- `cargo test` - Run all tests
- `cargo check` - Check for compilation errors without building
- `cargo clippy` - Run Rust linter for code quality checks

### Making Changes

- Run `cargo fmt` after changes to Rust code have compiled

### Running the Application

- `cargo run --bin main -- --help` - Show CLI help for the main generator
- `cargo run --bin lambda` - Run the Lambda function locally (requires AWS environment setup)

### Environment Variables

The application uses these environment variables:

- `LOL_SITE` - Site name (e.g., "burgerlist")
- `LOL_SITE_URL` - Site URL (e.g., "burgerl.ist")

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

**Executables**

- `main.rs` - CLI tool for local development and manual site generation
- `lambda.rs` - AWS Lambda function for automatic S3-triggered updates

### Data Flow

1. JSON configuration is read from S3 bucket or local file
2. HTML template (`index.template`) is loaded and processed
3. Data is validated and rendered through Minijinja templates
4. Generated HTML is optionally minified and written to destination

### S3 Integration

- Generator bucket: `{site_url}-generator` (stores JSON and templates)
- Site bucket: `{site_url}` (hosts the generated static site)
- Lambda function triggers on S3 object changes for automatic updates

### Terraform Infrastructure

The `list-of-lists.tf` file contains AWS infrastructure definitions for S3 buckets, Lambda functions, and associated IAM
roles.
