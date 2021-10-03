use anyhow::Result;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::error::Error;

pub static SITE_NAME_VAR: &str = "LOL_SITE";
pub static SITE_URL_VAR: &str = "LOL_SITE_URL";
pub static DB_KEY_VAR: &str = "LOL_DB_KEY";
pub static DB_PATH_VAR: &str = "LOL_DB_PATH";

pub type LambdaError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ListOfLists {
    pub title: String,
    pub lists: Vec<List>,
    pub card_image_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct List {
    pub title: String,

    #[serde(default)]
    pub hidden: bool,

    pub list: Vec<String>,
}

pub fn set_up_logger(verbose: bool) -> Result<()> {
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
        .chain(std::io::stdout())
        .apply();

    Ok(())
}

pub mod s3util {
    use super::*;
    use bytes::Bytes;
    use log::{debug, warn};
    use s3::{ByteStream, SdkError};

    pub async fn get(
        s3_client: &s3::Client,
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
        s3_client: &s3::Client,
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
        s3_client: &s3::Client,
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
