use crate::common::{self, ListOfLists};
use anyhow::Result;
use log::debug;
use rusoto_s3::S3Client;
use std::str;
use tera::{Context, Tera};
use tokio::fs;

const SITE_INDEX_TEMPLATE: &str = "index.template";
const SITE_INDEX: &str = "index.html";

enum Io {
    S3 {
        generator_bucket: String,
        site_bucket: String,
        s3_client: S3Client,
    },
    LocalFile,
}

impl Io {
    fn new(site_url: String, use_s3: bool) -> Self {
        if use_s3 {
            Self::S3 {
                generator_bucket: format!("{}-generator", site_url),
                site_bucket: site_url,
                s3_client: S3Client::new(Default::default()),
            }
        } else {
            Self::LocalFile
        }
    }

    async fn read(&self, target: &str) -> Result<String> {
        match self {
            Io::S3 {
                s3_client,
                generator_bucket,
                ..
            } => {
                let bytes = common::s3::get(s3_client, &generator_bucket, target).await?;
                Ok(str::from_utf8(&bytes)?.into())
            }

            Io::LocalFile => {
                debug!("Reading {}", target);
                Ok(fs::read_to_string(target).await?)
            }
        }
    }

    async fn write(&self, target: &str, content: impl AsRef<[u8]>) -> Result<()> {
        match self {
            Io::S3 {
                s3_client,
                site_bucket,
                ..
            } => common::s3::put(s3_client, site_bucket, target, "text/html", content).await?,

            Io::LocalFile => {
                debug!("Writing to {}", target);
                fs::write(target, content).await?
            }
        }

        Ok(())
    }

    async fn exists(&self, target: &str) -> Result<bool> {
        match self {
            Io::S3 {
                s3_client,
                site_bucket,
                ..
            } => common::s3::exists(s3_client, site_bucket, target).await,

            Io::LocalFile => {
                debug!("Checking if {} exists", target);
                Ok(fs::metadata(target).await.is_ok())
            }
        }
    }
}

async fn read_template(io: &Io, tera: &mut Tera) -> Result<()> {
    let template_content = io.read(SITE_INDEX_TEMPLATE).await?;
    tera.add_raw_template(SITE_INDEX, template_content.as_str())?;

    Ok(())
}

async fn read_list(io: &Io, site_name: &str) -> Result<ListOfLists> {
    let content = io.read(&format!("{}.json", site_name)).await?;
    let list_of_lists: ListOfLists = serde_json::from_str(content.as_str())?;

    Ok(list_of_lists)
}

async fn card_image_exists(io: &Io) -> Result<bool> {
    io.exists("images/card.png").await
}

pub async fn update_site(site_name: String, site_url: String, use_s3: bool) -> Result<()> {
    let mut tera = Tera::default();

    let io = Io::new(site_url, use_s3);

    let (_, mut list_of_lists, card_image_exists) = tokio::try_join!(
        read_template(&io, &mut tera),
        read_list(&io, &site_name),
        card_image_exists(&io),
    )?;

    list_of_lists.card_image_exists = card_image_exists;

    debug!("Rendering {}", SITE_INDEX);
    let site = tera.render(SITE_INDEX, &Context::from_serialize(list_of_lists)?)?;
    debug!("Rendered {}", SITE_INDEX);

    io.write(SITE_INDEX, &site).await
}
