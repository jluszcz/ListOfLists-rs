use crate::{s3util, ListOfLists};
use anyhow::Result;
use html5minify::Minify;
use log::debug;
use std::{
    path::{Path, PathBuf},
    str,
};
use tera::{Context, Tera};
use tokio::fs;

const SITE_INDEX_TEMPLATE: &str = "index.template";
const SITE_INDEX: &str = "index.html";

enum Io {
    S3 {
        s3_client: aws_sdk_s3::Client,
        generator_bucket: String,
        site_bucket: String,
    },
    LocalFile {
        path: PathBuf,
    },
}

impl Io {
    async fn new(site_url: String, use_s3: bool) -> Self {
        if use_s3 {
            let aws_config = aws_config::load_from_env().await;
            Self::S3 {
                s3_client: aws_sdk_s3::Client::new(&aws_config),
                generator_bucket: format!("{}-generator", site_url),
                site_bucket: site_url,
            }
        } else {
            Self::LocalFile {
                path: Path::new("buckets").join(site_url),
            }
        }
    }

    async fn read(&self, target: &str) -> Result<String> {
        match self {
            Io::S3 {
                s3_client,
                generator_bucket,
                ..
            } => {
                let bytes = s3util::get(s3_client, generator_bucket, target).await?;
                Ok(str::from_utf8(&bytes)?.into())
            }

            Io::LocalFile { path } => {
                let path = path.join(target);
                debug!("Reading {:?}", path);
                Ok(fs::read_to_string(path).await?)
            }
        }
    }

    async fn write(&self, target: &str, content: &[u8]) -> Result<()> {
        match self {
            Io::S3 {
                s3_client,
                site_bucket,
                ..
            } => s3util::put(s3_client, site_bucket, target, "text/html", content).await?,

            Io::LocalFile { path } => {
                let path = path.join(target);
                debug!("Writing to {:?}", path);
                fs::write(path, content).await?
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
            } => s3util::exists(s3_client, site_bucket, target).await,

            Io::LocalFile { path } => {
                let path = path.join(target);
                debug!("Checking if {:?} exists", path);
                Ok(fs::metadata(path).await.is_ok())
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

pub async fn update_site(
    site_name: String,
    site_url: String,
    use_s3: bool,
    minify: bool,
) -> Result<()> {
    let mut tera = Tera::default();

    let io = Io::new(site_url.clone(), use_s3).await;

    let (_, mut list_of_lists, card_image_exists) = tokio::try_join!(
        read_template(&io, &mut tera),
        read_list(&io, &site_name),
        card_image_exists(&io),
    )?;

    if card_image_exists {
        list_of_lists.card_image_url = Some(format!("https://{}/images/card.png", site_url));
    }

    debug!("Rendering {}", SITE_INDEX);
    let site = tera.render(SITE_INDEX, &Context::from_serialize(list_of_lists)?)?;
    debug!("Rendered {}", SITE_INDEX);

    let site = if minify {
        let original_size = site.len();
        debug!(
            "Minifying {} (original size: {})",
            SITE_INDEX, original_size
        );

        let site = site.minify()?;

        debug!(
            "Minified {}: {:.1}% (new size: {})",
            SITE_INDEX,
            100.0 * (site.len() as f64 / original_size as f64),
            site.len()
        );

        site
    } else {
        site.as_bytes().to_vec()
    };

    io.write(SITE_INDEX, &site).await
}
