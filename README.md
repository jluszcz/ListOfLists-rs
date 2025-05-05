# ListOfLists

ListOfLists can generate a static website, hosted on AWS in an S3 bucket, from a `json` file stored in S3.

## Status

[![Status Badge](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/build-and-deploy.yml/badge.svg)](https://github.com/jluszcz/ListOfLists-rs/actions/workflows/build-and-deploy.yml)

## List JSON

```
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
    "footerLinks": [
        {
            "url": "https://github.com/jluszcz/ListOfLists-rs",
            "icon": "github"
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

### Footers

The `footerLinks` list in the example above will use `icon` as a [Bootstrap Icon](https://icons.getbootstrap.com). The
`footer` object in the example above is more generic - you can use `imports` and `links` to use either
[Bootstrap Icons](https://icons.getbootstrap.com) or [Font Awesome](https://fontawesome.com), and `icon` will be passed
as the CSS class of the icon.

When both are present, the newer `footer` object will be used.

## Update Site

### Helper Script

```
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

1. Upload a `${LOL_SITE}.json` to `s3://${LOL_SITE_URL}-generator/${LOL_SITE}.json`

- See [moviel.ist](https://github.com/jluszcz/MovieList) or [burgerl.ist](https://github.com/jluszcz/BurgerList) for
  examples of how to automate this with [GitHub actions](https://github.com/features/actions).
