#  ListOfLists

ListOfLists can generate a static website, hosted on AWS in an S3 bucket, from a `json` file stored in S3.

## Status

[![Build Status](https://app.travis-ci.com/jluszcz/ListOfLists-rs.svg?branch=main)](https://app.travis-ci.com/jluszcz/ListOfLists-rs)

## List JSON

```
{
    "title": "The List",
    "footerLinks": [
        {
            "url": "https://github.com/jluszcz/ListOfLists-rs",
            "icon": "github"
        }
    ]
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
    ]
}
```

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
