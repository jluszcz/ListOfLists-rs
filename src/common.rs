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
        .level(level)
        .level_for("hyper", LevelFilter::Off)
        .level_for("rustls", LevelFilter::Off)
        .level_for("smithy_http_tower", LevelFilter::Off)
        .level_for("tracing", LevelFilter::Off)
        .level_for("reqwest", LevelFilter::Off)
        .chain(std::io::stdout())
        .apply();

    Ok(())
}

pub mod s3 {
    use super::*;
    use bytes::Bytes;
    use log::debug;
    use rusoto_s3::{GetObjectRequest, PutObjectRequest, S3Client, S3};
    use tokio::io::AsyncReadExt;

    pub async fn get(s3_client: &S3Client, bucket_name: &str, object_name: &str) -> Result<Bytes> {
        let request = GetObjectRequest {
            bucket: bucket_name.into(),
            key: object_name.into(),
            ..Default::default()
        };

        let mut bytes = Vec::new();

        debug!("Reading {}:{} from S3", bucket_name, object_name);
        s3_client
            .get_object(request)
            .await?
            .body
            .expect("no body on response")
            .into_async_read()
            .read_to_end(&mut bytes)
            .await?;
        debug!("Read {}:{} from S3", bucket_name, object_name);

        Ok(Bytes::from(bytes))
    }

    pub async fn put(
        s3_client: &S3Client,
        bucket_name: &str,
        object_name: &str,
        content_type: &str,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        let request = PutObjectRequest {
            bucket: bucket_name.into(),
            key: object_name.into(),
            content_type: Some(content_type.into()),
            body: Some(Vec::from(data.as_ref()).into()),
            ..Default::default()
        };

        debug!("Uploading {}:{} to S3", bucket_name, object_name);
        s3_client.put_object(request).await?;
        debug!("Uploaded {}:{} to S3", bucket_name, object_name);

        Ok(())
    }
}
