#  ListOfLists

ListOfLists can generate a static website, hosted on AWS in an S3 bucket, from a json file stored in Dropbox.

## Status

[![Build Status](https://www.travis-ci.com/jluszcz/ListOfLists-rs.svg?branch=main)](https://travis-ci.com/jluszcz/ListOfLists-rs)

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

export LOL_DB_KEY="1234ABCD"
export LOL_DB_PATH="/MyDirectory/$LOL_SITE.json"

export TF_VAR_aws_acct_id="123412341234"
export TF_VAR_site_name=${LOL_SITE}
export TF_VAR_site_url=${LOL_SITE_URL}
export TF_VAR_db_access_key=${LOL_DB_KEY}
export TF_VAR_db_file_path=${LOL_DB_PATH}
```

### Update

1. `source env-helper`
1. _Build_
1. `terraform apply`
