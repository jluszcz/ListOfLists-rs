use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub mod generator;

pub const APP_NAME: &str = "list_of_lists";

pub const SITE_NAME_VAR: &str = "LOL_SITE";
pub const SITE_URL_VAR: &str = "LOL_SITE_URL";

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct ListOfLists {
    pub title: String,
    pub lists: Vec<List>,

    #[serde(default, alias = "footerLinks")]
    pub footer_links: Vec<FooterItem>,

    pub footer: Option<Footer>,
}

impl ListOfLists {
    pub fn validate(self) -> Result<Self> {
        for l in &self.lists {
            l.validate()?;
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct List {
    pub title: String,

    #[serde(default)]
    pub hidden: bool,

    #[serde(default)]
    duplicates: bool,

    pub list: Vec<ListItem>,
}

impl List {
    fn validate(&self) -> Result<()> {
        if !self.duplicates && self.list.iter().collect::<HashSet<_>>().len() != self.list.len() {
            Err(anyhow!("Illegal duplicates found in {:?}", self.list))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListItem {
    Item(String),
    WithTooltip { item: String, tooltip: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct Footer {
    #[serde(default)]
    pub imports: Vec<String>,

    #[serde(default)]
    pub links: Vec<FooterItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct FooterItem {
    pub url: String,
    pub icon: String,

    #[serde(default)]
    pub title: Option<String>,
}

pub mod s3util {
    use super::*;
    use aws_sdk_s3::primitives::ByteStream;
    use bytes::Bytes;
    use log::debug;

    pub async fn get(
        s3_client: &aws_sdk_s3::Client,
        bucket_name: &str,
        object_name: &str,
    ) -> Result<Bytes> {
        debug!("Reading {bucket_name}:{object_name} from S3");
        let bytes = s3_client
            .get_object()
            .bucket(bucket_name)
            .key(object_name)
            .send()
            .await?
            .body
            .collect()
            .await?
            .into_bytes();
        debug!("Read {bucket_name}:{object_name} from S3");

        Ok(bytes)
    }

    pub async fn put(
        s3_client: &aws_sdk_s3::Client,
        bucket_name: &str,
        object_name: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<()> {
        debug!("Uploading {bucket_name}:{object_name} to S3");
        s3_client
            .put_object()
            .bucket(bucket_name)
            .key(object_name)
            .content_type(content_type)
            .body(ByteStream::from(Bytes::from(Vec::from(data))))
            .send()
            .await?;
        debug!("Uploaded {bucket_name}:{object_name} to S3");

        Ok(())
    }

    pub async fn exists(
        s3_client: &aws_sdk_s3::Client,
        bucket_name: &str,
        object_name: &str,
    ) -> Result<bool> {
        debug!("Checking {bucket_name}:{object_name} on S3");
        let response = s3_client
            .head_object()
            .bucket(bucket_name)
            .key(object_name)
            .send()
            .await;
        debug!("Checked {bucket_name}:{object_name} on S3");

        Ok(response.is_ok())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const EXAMPLE_LIST: &str = r#"
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
                "title": "Tooltip",
                "list": [
                    "foo",
                    {
                        "item": "bar",
                        "tooltip": "baz"
                    }
                ]
            }
        ]
    }
    "#;

    impl List {
        fn new(title: &str, hidden: bool, duplicates: bool, list: &[&str]) -> Self {
            let list_items: Vec<ListItem> = list.iter().cloned().map(ListItem::new).collect();
            Self::from_items(title, hidden, duplicates, list_items)
        }

        fn from_items(title: &str, hidden: bool, duplicates: bool, list: Vec<ListItem>) -> Self {
            Self {
                title: title.to_string(),
                hidden,
                duplicates,
                list,
            }
        }
    }

    impl ListItem {
        fn new(item: &str) -> Self {
            ListItem::Item(item.to_string())
        }

        fn with_tooltip(item: &str, tooltip: &str) -> Self {
            ListItem::WithTooltip {
                item: item.to_string(),
                tooltip: tooltip.to_string(),
            }
        }
    }

    #[test]
    fn test_list_of_lists_serde() -> Result<()> {
        let list_of_lists = ListOfLists {
            title: "The List".to_string(),
            footer_links: vec![],
            footer: None,
            lists: vec![
                List::new("Letters", true, false, &["A", "B", "C"]),
                List::new("Numbers", false, false, &["1", "2", "3"]),
                List::from_items(
                    "Tooltip",
                    false,
                    false,
                    vec![ListItem::new("foo"), ListItem::with_tooltip("bar", "baz")],
                ),
            ],
        };

        let serialized = serde_json::to_string(&list_of_lists)?;
        let deserialized: ListOfLists = serde_json::from_str(&serialized)?;
        let from_example: ListOfLists = serde_json::from_str(EXAMPLE_LIST)?;

        assert_eq!(list_of_lists, deserialized);
        assert_eq!(list_of_lists, from_example);

        Ok(())
    }

    #[test]
    fn test_list_validation_duplicates_allowed() {
        let l = List::new("Letters", false, true, &["A", "A"]);

        assert!(l.validate().is_ok());
    }

    #[test]
    fn test_list_validation_duplicates_disallowed() {
        let l = List::new("Letters", false, false, &["A", "A"]);

        assert!(l.validate().is_err());
    }

    #[test]
    fn test_list_of_lists_footer() -> Result<()> {
        let list_of_lists = ListOfLists {
            title: "The List".to_string(),
            footer_links: vec![],
            footer: Some(Footer {
                imports: vec!["https://import.js".to_string()],
                links: vec![FooterItem {
                    url: "https://github.com".to_string(),
                    icon: "github".to_string(),
                    title: Some("GitHub".to_string()),
                }],
            }),
            lists: vec![List::new("Letters", true, false, &["A", "B", "C"])],
        };

        let serialized = serde_json::to_string(&list_of_lists)?;
        let deserialized: ListOfLists = serde_json::from_str(&serialized)?;

        assert_eq!(list_of_lists, deserialized);

        Ok(())
    }

    #[test]
    fn test_list_of_lists_legacy_footer() -> Result<()> {
        let list_of_lists = ListOfLists {
            title: "The List".to_string(),
            footer_links: vec![FooterItem {
                url: "https://github.com".to_string(),
                icon: "github".to_string(),
                title: None,
            }],
            footer: None,
            lists: vec![List::new("Letters", true, false, &["A", "B", "C"])],
        };

        let serialized = serde_json::to_string(&list_of_lists)?;
        let deserialized: ListOfLists = serde_json::from_str(&serialized)?;

        assert_eq!(list_of_lists, deserialized);

        Ok(())
    }
}
