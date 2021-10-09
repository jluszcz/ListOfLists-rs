use crate::s3util;
use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use log::{debug, info, trace};
use reqwest::header::HeaderMap;
use serde::Serialize;
use serde_json::{ser, Value};
use std::convert::TryFrom;
use tokio::try_join;

#[derive(Debug, Serialize)]
struct DropboxGetFileMetadataBody {
    path: String,
    include_media_info: bool,
    include_deleted: bool,
    include_has_explicit_shared_members: bool,
}

impl DropboxGetFileMetadataBody {
    fn from_path<T>(path: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            path: path.into(),
            include_media_info: false,
            include_deleted: false,
            include_has_explicit_shared_members: false,
        }
    }
}

impl TryFrom<DropboxGetFileMetadataBody> for String {
    type Error = serde_json::Error;

    fn try_from(value: DropboxGetFileMetadataBody) -> Result<Self, Self::Error> {
        serde_json::to_string(&value)
    }
}

#[derive(Debug, Serialize)]
struct DropboxGetFileBody {
    path: String,
}

impl DropboxGetFileBody {
    fn from_path<T>(path: T) -> Self
    where
        T: Into<String>,
    {
        Self { path: path.into() }
    }
}

impl TryFrom<DropboxGetFileBody> for String {
    type Error = serde_json::Error;

    fn try_from(value: DropboxGetFileBody) -> Result<Self, Self::Error> {
        serde_json::to_string(&value)
    }
}

async fn get_dropbox_metadata(dropbox_key: &str, dropbox_path: &str) -> Result<DateTime<Utc>> {
    let url = "https://api.dropboxapi.com/2/files/get_metadata";

    let mut headers = HeaderMap::with_capacity(2);
    headers.insert("Authorization", format!("Bearer {}", dropbox_key).parse()?);
    headers.insert("Content-Type", "application/json".parse()?);

    let body = String::try_from(DropboxGetFileMetadataBody::from_path(dropbox_path))?;

    let client = reqwest::Client::new();

    trace!("Querying Dropbox:: '{}'", body);
    let response = client
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .await?
        .text()
        .await?;
    trace!("Dropbox Response: {}", response);

    let response: Value = serde_json::from_str(response.as_str())?;

    let last_modified_time = response["client_modified"]
        .as_str()
        .ok_or_else(|| anyhow!("missing client_modified"))?;
    let last_modified_time = NaiveDateTime::parse_from_str(last_modified_time, "%FT%TZ")?;
    let last_modified_time = DateTime::<Utc>::from_utc(last_modified_time, Utc);

    debug!(
        "{} last modified time: {:?}",
        dropbox_path, last_modified_time
    );

    Ok(last_modified_time)
}

async fn get_s3_metadata(
    s3_client: &s3::Client,
    bucket_name: &str,
    object_name: &str,
) -> Result<(String, DateTime<Utc>)> {
    trace!("Querying S3");
    let response = s3_client
        .head_object()
        .bucket(bucket_name)
        .key(object_name)
        .send()
        .await?;
    trace!("S3 Response: {:?}", response);

    let e_tag = response
        .e_tag
        .map(|e| e.replace(r#"""#, ""))
        .ok_or_else(|| anyhow!("missing e_tag"))?;

    let last_modified_time = response
        .last_modified
        .map(|t| t.to_chrono())
        .ok_or_else(|| anyhow!("missing last_modified"))?;

    debug!(
        "{} e_tag: {}, last modified time: {}",
        object_name, e_tag, last_modified_time
    );

    Ok((e_tag, last_modified_time))
}

async fn get_list_from_dropbox(dropbox_key: &str, dropbox_path: &str) -> Result<(String, Vec<u8>)> {
    let url = "https://content.dropboxapi.com/2/files/download";

    let mut headers = HeaderMap::with_capacity(1);
    headers.insert("Authorization", format!("Bearer {}", dropbox_key).parse()?);

    let body = String::try_from(DropboxGetFileBody::from_path(dropbox_path))?;

    let client = reqwest::Client::new();

    trace!("Querying Dropbox:: '{}'", body);
    let response: Value = client
        .post(url)
        .headers(headers)
        .query(&[("arg", body)])
        .send()
        .await?
        .json()
        .await?;
    trace!("Dropbox Response: {:?}", response);

    let file_content = ser::to_vec(&response)?;
    let digest = format!("{:x}", md5::compute(&file_content));

    Ok((digest, file_content))
}

pub async fn try_update_list_file(
    site_name: String,
    site_url: String,
    dropbox_key: String,
    dropbox_path: String,
    force: bool,
) -> Result<()> {
    let aws_config = aws_config::load_from_env().await;
    let s3_client = s3::Client::new(&aws_config);

    let s3_bucket_name = format!("{}-generator", site_url);
    let s3_object_name = format!("{}.json", site_name);

    let (db_last_modified_time, (e_tag, s3_last_modified_time)) = try_join!(
        get_dropbox_metadata(&dropbox_key, &dropbox_path),
        get_s3_metadata(&s3_client, &s3_bucket_name, &s3_object_name)
    )?;

    if db_last_modified_time <= s3_last_modified_time && !force {
        info!(
            "{} has not been modified since the last S3 upload at {}, skipping",
            s3_object_name, s3_last_modified_time,
        );
        Ok(())
    } else {
        let (db_md5, list) = get_list_from_dropbox(&dropbox_key, &dropbox_path).await?;

        debug!("Dropbox MD5: {}, S3 ETag: {}", db_md5, e_tag);
        if db_md5 == e_tag && !force {
            info!("{} is already up to date, skipping", s3_object_name);
        } else {
            s3util::put(
                &s3_client,
                &s3_bucket_name,
                &s3_object_name,
                "application/json",
                &list,
            )
            .await?;
        }

        Ok(())
    }
}
