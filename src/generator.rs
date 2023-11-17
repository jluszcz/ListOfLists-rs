use crate::{s3util, ListOfLists};
use anyhow::Result;
use aws_config::ConfigLoader;
use html5minify::Minify;
use log::debug;
use minijinja::{Environment, Error, State};
use regex::Regex;
use std::sync::OnceLock;
use std::{
    path::{Path, PathBuf},
    str,
};
use tokio::fs;

const SITE_INDEX_TEMPLATE: &str = "index.template";
const SITE_INDEX: &str = "index.html";

const DIV_ID_SAFE: &str = "div_id_safe";

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
            let aws_config = ConfigLoader::default().load().await;
            Self::S3 {
                s3_client: aws_sdk_s3::Client::new(&aws_config),
                generator_bucket: format!("{site_url}-generator"),
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
                let res = fs::read_to_string(&path).await;
                debug!(
                    "{} {:?}",
                    if res.is_ok() {
                        "Read"
                    } else {
                        "Failed to read"
                    },
                    path
                );
                Ok(res?)
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
                let res = fs::write(&path, content).await;
                debug!(
                    "{} {:?}",
                    if res.is_ok() {
                        "Wrote"
                    } else {
                        "Failed to write"
                    },
                    path
                );
                res?
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

async fn read_template(io: &Io) -> Result<String> {
    io.read(SITE_INDEX_TEMPLATE).await
}

async fn read_list(io: &Io, site_name: &str) -> Result<ListOfLists> {
    let content = io.read(&format!("{site_name}.json")).await?;
    let list_of_lists: ListOfLists = serde_json::from_str(content.as_str())?;

    Ok(list_of_lists)
}

async fn card_image_exists(io: &Io) -> Result<bool> {
    io.exists("images/card.png").await
}

fn div_id_safe(_: &State, value: String) -> Result<String, Error> {
    Ok(inner_div_id_safe(value))
}

fn inner_div_id_safe<S>(value: S) -> String
where
    S: Into<String>,
{
    static RE: OnceLock<Regex> = OnceLock::new();

    RE.get_or_init(|| Regex::new("[^[_0-9A-Za-z]]").unwrap())
        .replace_all(&value.into().replace(' ', "_"), "")
        .into_owned()
}

pub async fn update_site(
    site_name: String,
    site_url: String,
    use_s3: bool,
    minify: bool,
) -> Result<()> {
    let io = Io::new(site_url.clone(), use_s3).await;

    let (template, mut list_of_lists, card_image_exists) = tokio::try_join!(
        read_template(&io),
        read_list(&io, &site_name),
        card_image_exists(&io),
    )?;

    if card_image_exists {
        list_of_lists.card_image_url = Some(format!("https://{site_url}/images/card.png"));
    }

    let mut env = Environment::new();
    env.add_template(SITE_INDEX, &template)?;
    env.add_filter(DIV_ID_SAFE, div_id_safe);

    let template = env.get_template(SITE_INDEX)?;

    debug!("Rendering {}", SITE_INDEX);
    let site = template.render(&list_of_lists)?;
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_div_id_safe() {
        assert_eq!("foo_bar_baz", inner_div_id_safe("foo, bar, baz"));
        assert_eq!("Foo_Bar_Baz", inner_div_id_safe("Foo, Bar, Baz"));
        assert_eq!("foo_1234", inner_div_id_safe("foo 1234"));
        assert_eq!("Foo_1234", inner_div_id_safe("Foo 1234"));
        assert_eq!("1234", inner_div_id_safe("1234"));
    }
}
