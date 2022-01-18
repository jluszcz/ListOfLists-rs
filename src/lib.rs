use anyhow::Result;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub mod generator;

pub static SITE_NAME_VAR: &str = "LOL_SITE";
pub static SITE_URL_VAR: &str = "LOL_SITE_URL";

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct ListOfLists {
    pub title: String,
    pub lists: Vec<List>,
    pub card_image_url: Option<String>,
}

impl ListOfLists {
    fn remove_hidden(&mut self) {
        let mut i = 0;
        while i < self.lists.len() {
            if self.lists[i].hidden {
                let _ = self.lists.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct List {
    pub title: String,

    #[serde(default)]
    pub hidden: bool,

    pub list: Vec<String>,
}

pub fn set_up_logger<T>(calling_module: T, verbose: bool) -> Result<()>
where
    T: Into<Cow<'static, str>>,
{
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let _ = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(LevelFilter::Warn)
        .level_for("list_of_lists", level)
        .level_for(calling_module, level)
        .chain(std::io::stdout())
        .apply();

    Ok(())
}

pub mod s3util {
    use super::*;
    use aws_sdk_s3::{ByteStream, SdkError};
    use bytes::Bytes;
    use log::{debug, warn};

    pub async fn get(
        s3_client: &aws_sdk_s3::Client,
        bucket_name: &str,
        object_name: &str,
    ) -> Result<Bytes> {
        debug!("Reading {}:{} from S3", bucket_name, object_name);
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
        debug!("Read {}:{} from S3", bucket_name, object_name);

        Ok(bytes)
    }

    pub async fn put(
        s3_client: &aws_sdk_s3::Client,
        bucket_name: &str,
        object_name: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<()> {
        debug!("Uploading {}:{} to S3", bucket_name, object_name);
        s3_client
            .put_object()
            .bucket(bucket_name)
            .key(object_name)
            .content_type(content_type)
            .body(ByteStream::from(Bytes::from(Vec::from(data))))
            .send()
            .await?;
        debug!("Uploaded {}:{} to S3", bucket_name, object_name);

        Ok(())
    }

    pub async fn exists(
        s3_client: &aws_sdk_s3::Client,
        bucket_name: &str,
        object_name: &str,
    ) -> Result<bool> {
        debug!("Checking {}:{} on S3", bucket_name, object_name);
        let response = s3_client
            .head_object()
            .bucket(bucket_name)
            .key(object_name)
            .send()
            .await;
        debug!("Checked {}:{} on S3", bucket_name, object_name);

        Ok(match response {
            Ok(_) => true,
            Err(SdkError::ServiceError { err, .. }) => !err.is_not_found(),
            _ => {
                warn!("Failed to query S3: {:?}", response);
                false
            }
        })
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
            }
        ]
    }
    "#;

    impl List {
        fn new(title: &str, hidden: bool, list: &[&str]) -> Self {
            Self {
                title: title.to_string(),
                hidden,
                list: list.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    #[test]
    fn test_list_of_lists_serde() -> Result<()> {
        let list_of_lists = ListOfLists {
            title: "The List".to_string(),
            card_image_url: None,
            lists: vec![
                List::new("Letters", true, &vec!["A", "B", "C"]),
                List::new("Numbers", false, &vec!["1", "2", "3"]),
            ],
        };

        let deserialized = serde_json::from_str(EXAMPLE_LIST)?;

        assert_eq!(list_of_lists, deserialized);

        Ok(())
    }
}
