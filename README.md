# ListOfLists

ListOfLists generates a static website, hosted on AWS in an S3 bucket, from a JSON file stored in S3.

## Status

[![Status Badge](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/build-and-deploy.yml/badge.svg)](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/build-and-deploy.yml)

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

| Field | Type | Default | Description |
|---|---|---|---|
| `title` | string | required | Display title for the list |
| `hidden` | bool | `false` | If `true`, the list is hidden by default |
| `duplicates` | bool | `false` | If `false`, duplicate items cause a validation error |
| `list` | array | required | Array of items (strings or objects with `item`/`tooltip`) |

### Footers

The `footer` object supports `imports` and `links`. Use `imports` to inject `<script>` or `<link>` tags (e.g. for icon libraries), and `links` to render footer icons. The `icon` field is passed as a CSS class, so it works with both [Bootstrap Icons](https://icons.getbootstrap.com) and [Font Awesome](https://fontawesome.com).

The legacy `footerLinks` top-level array is still supported for backwards compatibility; `icon` in that format is treated as a [Bootstrap Icon](https://icons.getbootstrap.com) name. When both `footerLinks` and `footer` are present, `footer` takes precedence.

## Local Development

Files are read from `buckets/{site_url}/` when running locally. The directory must contain:

- `index.template` — Minijinja HTML template
- `{site_name}.json` — List data file

Run the generator locally:

```sh
cargo run --bin main -- --site-name <site_name> --site-url <site_url>
```

### CLI Flags

| Flag | Env Var | Description |
|---|---|---|
| `-s`, `--site-name` | `LOL_SITE` | Site name (e.g. `burgerlist`) |
| `-u`, `--site-url` | `LOL_SITE_URL` | Site URL (e.g. `burgerl.ist`) |
| `-r`, `--remote` | | Use S3 instead of local files |
| `-m`, `--minify` | | Minify the generated HTML |
| `-v` / `-vv` | | Enable DEBUG / TRACE logging |

## Deploying to AWS

### Helper Script

```sh
#!/usr/bin/env sh

export LOL_SITE_URL="list-of-l.ist"
export LOL_SITE=$(echo ${LOL_SITE_URL} | sed 's/\.//')

export TF_VAR_aws_acct_id="123412341234"
export TF_VAR_site_name=${LOL_SITE}
export TF_VAR_site_url=${LOL_SITE_URL}
```

### Update AWS Configuration

1. `source env-helper`
1. _Build_
1. `terraform apply`

### Update List

1. Upload `${LOL_SITE}.json` to `s3://${LOL_SITE_URL}-generator/${LOL_SITE}.json`

The Lambda function triggers automatically on S3 object changes to regenerate the site.

See [moviel.ist](https://github.com/jluszcz/MovieList) or [burgerl.ist](https://github.com/jluszcz/BurgerList) for
examples of how to automate uploads with [GitHub Actions](https://github.com/features/actions).
