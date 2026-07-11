# ListOfLists

ListOfLists generates a static website, hosted on AWS in an S3 bucket, from a JSON file stored in S3.

## Status

[![.github/workflows/ci.yml](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/ci.yml)

[![.github/workflows/update-index-template.yml](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/update-index-template.yml/badge.svg)](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/update-index-template.yml)

## List JSON

```json
{
  "title": "The List",
  "description": "An example list of lists",
  "lists": [
    {
      "title": "Letters",
      "hidden": true,
      "list": [
        "A",
        "B",
        "C"
      ]
    },
    {
      "title": "Numbers",
      "list": [
        "1",
        "2",
        "3"
      ]
    },
    {
      "title": "With Tooltips",
      "list": [
        "Foo",
        "Bar",
        "Baz",
        {
          "item": "Quux",
          "tooltip": "Not many people know this one"
        }
      ]
    }
  ],
  "footer": {
    "imports": [
      "<script src=\"https://kit.fontawesome.com/deadbeef.js\" crossorigin=\"anonymous\"></script>"
    ],
    "links": [
      {
        "url": "https://github.com/jluszcz/ListOfLists-rs",
        "title": "GitHub",
        "icon": "fa-brands fa-github"
      }
    ]
  }
}
```

The optional top-level `description` is used for the page's meta and OpenGraph descriptions; it falls back to `title`
when omitted.

### List Fields

| Field        | Type   | Default  | Description                                               |
|--------------|--------|----------|-----------------------------------------------------------|
| `title`      | string | required | Display title for the list                                |
| `hidden`     | bool   | `false`  | If `true`, the list is hidden by default                  |
| `duplicates` | bool   | `false`  | If `false`, duplicate items cause a validation error      |
| `list`       | array  | required | Array of items (strings or objects with `item`/`tooltip`) |

### Validation

The generator rejects input that would produce a degenerate page:

- The top-level `title` must be non-empty, and `description` (if present) must be non-empty.
- `lists` must contain at least one list.
- Each list `title`, each item string, and each tooltip must be non-empty.
- Duplicate items within a list are rejected unless `duplicates: true`.
- Visible list titles must remain distinct after sanitization into HTML ids (e.g. `Foo Bar` and `Foo_Bar` collide),
  and must contain at least one usable id character (`A-Z`, `a-z`, `0-9`, `_`).

### Footers

The `footer` object supports `imports` and `links`. Use `imports` to inject `<script>` or `<link>` tags (e.g. for icon
libraries), and `links` to render footer icons. The `icon` field is passed as a CSS class, so it works with
both [Bootstrap Icons](https://icons.getbootstrap.com) and [Font Awesome](https://fontawesome.com).

The legacy `footerLinks` top-level array is still supported for backwards compatibility; `icon` in that format is
treated as a [Bootstrap Icon](https://icons.getbootstrap.com) name. When both `footerLinks` and `footer` are present,
`footer` takes precedence.

## Local Development

Files are read from `buckets/{generator_bucket}/` when running locally (default: `buckets/generator/`). The directory
must contain:

- `index.template` — Minijinja HTML template
- `{site_url}.json` — List data file

Run the generator locally:

```sh
cargo run --bin main -- --site-url <site_url>
```

### CLI Flags

| Flag                       | Env Var                | Default     | Description                   |
|----------------------------|------------------------|-------------|-------------------------------|
| `-u`, `--site-url`         | `LOL_SITE_URL`         | required    | Site URL (e.g. `list-of-l.ist`) |
| `-g`, `--generator-bucket` | `LOL_GENERATOR_BUCKET` | `generator` | Generator bucket name         |
| `-r`, `--remote`           |                        |             | Use S3 instead of local files |
| `-m`, `--minify`           |                        |             | Minify the generated HTML     |
| `-v` / `-vv`               |                        |             | Enable DEBUG / TRACE logging  |

## Deploying to AWS

The Terraform configuration is split into two parts, each with its own backend state:

- `shared/` — account-wide resources (generator bucket, Lambda, shared IAM roles). Apply once per AWS account.
- `site/` — per-site resources (site bucket, CloudFront, Route53, ACM, GitHub OIDC role). Apply once per site.

### Shared Infrastructure

Apply from `shared/` (state key is fixed at `list-of-lists/shared`):

```sh
cd shared
terraform init
terraform apply
```

### Per-Site Infrastructure

Each site needs its own Terraform state, keyed off the site URL. Site identities are kept **out of the repo**: the
reusable resources live in `site-module/`, and each site is a small, gitignored root module under `sites/<site_url>/`
that calls it. Scaffold one with the helper script (no repo changes needed to add a site):

```sh
scripts/new-site.sh <site_url> <github_org> <github_repo> [site_name]
# e.g. scripts/new-site.sh list-of-l.ist jluszcz ListOfL.ist
```

This writes `sites/<site_url>/main.tf` (backend key `list-of-lists/sites/<site_url>`, calling `../../site-module`)
and a `.envrc`. Then:

```sh
cd sites/<site_url>
direnv allow          # first time only; loads the site's LOL_* vars
terraform init
terraform apply
```

Switching sites is just `cd` — each workspace has its own state and direnv context, so there is no
`-backend-config` juggling or re-init between sites.

### Update List

1. Upload `${LOL_SITE_URL}.json` to `s3://<generator_bucket>/${LOL_SITE_URL}.json`

The Lambda function triggers automatically on S3 object changes:

- A change to `${site_url}.json` regenerates that single site.
- A change to `index.template` regenerates every site found in the generator bucket. Sites are rendered concurrently
  using a shared parsed template.

After each render, the Lambda issues a CloudFront invalidation for `/index.html` on the distribution whose aliases
include the site URL. Distribution lookups are cached for the lifetime of the warm container. Invalidation failures are
logged but do not fail the Lambda; the new `index.html` is already in S3 and will be served once the existing cache
entry expires.

### Lambda IAM

The Lambda role (defined in `shared/main.tf`) requires:

- `s3:GetObject` and `s3:ListBucket` on the generator bucket.
- `s3:PutObject` on `arn:aws:s3:::*/index.html` (broad by design — see comment in `shared/main.tf`).
- `cloudfront:ListDistributions` and `cloudfront:CreateInvalidation` (resource `*`) for the post-render invalidation.

Re-apply `shared/` Terraform when upgrading from a version without CloudFront permissions.

A site's own repo can automate uploading `${LOL_SITE_URL}.json` to the generator bucket with
[GitHub Actions](https://github.com/features/actions), assuming the `*.github-update` role created by `site-module`.
