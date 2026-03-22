# ListOfLists

ListOfLists generates a static website, hosted on AWS in an S3 bucket, from a JSON file stored in S3.

## Status

[![Status Badge](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/build-and-deploy.yml/badge.svg)](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/build-and-deploy.yml)
[![.github/workflows/update-index-template.yml](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/update-index-template.yml/badge.svg)](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/update-index-template.yml)

## List JSON

```json
{
  "title": "The List",
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

### List Fields

| Field        | Type   | Default  | Description                                               |
|--------------|--------|----------|-----------------------------------------------------------|
| `title`      | string | required | Display title for the list                                |
| `hidden`     | bool   | `false`  | If `true`, the list is hidden by default                  |
| `duplicates` | bool   | `false`  | If `false`, duplicate items cause a validation error      |
| `list`       | array  | required | Array of items (strings or objects with `item`/`tooltip`) |

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
| `-u`, `--site-url`         | `LOL_SITE_URL`         | required    | Site URL (e.g. `burgerl.ist`) |
| `-g`, `--generator-bucket` | `LOL_GENERATOR_BUCKET` | `generator` | Generator bucket name         |
| `-r`, `--remote`           |                        |             | Use S3 instead of local files |
| `-m`, `--minify`           |                        |             | Minify the generated HTML     |
| `-v` / `-vv`               |                        |             | Enable DEBUG / TRACE logging  |

## Deploying to AWS

### Helper Script

```sh
#!/usr/bin/env sh

export LOL_SITE_URL="list-of-l.ist"

export TF_VAR_site_url=${LOL_SITE_URL}
```

### Update AWS Configuration

1. `source env-helper`
1. _Build_
1. `terraform apply`

### Update List

1. Upload `${LOL_SITE_URL}.json` to `s3://<generator_bucket>/${LOL_SITE_URL}.json`

The Lambda function triggers automatically on S3 object changes to regenerate the site.

See [moviel.ist](https://github.com/jluszcz/MovieList) or [burgerl.ist](https://github.com/jluszcz/BurgerList) for
examples of how to automate uploads with [GitHub Actions](https://github.com/features/actions).
