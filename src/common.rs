use std::error::Error;

use anyhow::Result;
use log::LevelFilter;
use serde::{Deserialize, Serialize};

pub static SITE_NAME_VAR: &str = "LOL_SITE";
pub static SITE_URL_VAR: &str = "LOL_SITE_URL";
pub static DB_KEY_VAR: &str = "LOL_DB_KEY";
pub static DB_PATH_VAR: &str = "LOL_DB_PATH";

pub type LambdaError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ListOfLists {
    pub title: String,
    pub lists: Vec<List>,
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

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(level)
        .level_for("hyper", LevelFilter::Off)
        .level_for("rustls", LevelFilter::Off)
        .level_for("smithy_http_tower", LevelFilter::Off)
        .level_for("tracing", LevelFilter::Off)
        .level_for("reqwest", LevelFilter::Off)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}

pub mod s3 {
    use super::*;
    use ::s3::{self, ByteStream};
    use bytes::Bytes;
    use log::debug;

    pub async fn get(
        s3_client: &s3::Client,
        bucket_name: &str,
        object_name: &str,
    ) -> Result<Bytes> {
        let request = s3_client.get_object().bucket(bucket_name).key(object_name);

        debug!("Reading {}:{} from S3", bucket_name, object_name);
        let response = request.send().await?;
        let bytes = response.body.collect().await?.into_bytes();
        debug!("Read {}:{} from S3", bucket_name, object_name);

        Ok(bytes)
    }

    pub async fn put(
        s3_client: &s3::Client,
        bucket_name: &str,
        object_name: &str,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        let request = s3_client
            .put_object()
            .bucket(bucket_name)
            .key(object_name)
            .body(ByteStream::from(Vec::from(data.as_ref())));

        debug!("Uploading {}:{} to S3", bucket_name, object_name);
        request.send().await?;
        debug!("Uploaded {}:{} to S3", bucket_name, object_name);

        Ok(())
    }
}
