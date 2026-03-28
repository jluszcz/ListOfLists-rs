use crate::{ListOfLists, s3util};
use anyhow::Result;
use log::{debug, trace};
use minify_html::Cfg;
use minijinja::{Environment, Error, State};
use regex::Regex;
use std::sync::LazyLock;
use std::{
    path::{Path, PathBuf},
    str,
};
use tokio::fs;

pub const SITE_INDEX_TEMPLATE: &str = "index.template";
const SITE_INDEX: &str = "index.html";

const DIV_ID_SAFE: &str = "div_id_safe";

enum Io {
    S3 {
        s3_client: aws_sdk_s3::Client,
        generator_bucket: String,
        site_bucket: String,
    },
    LocalFile {
        generator_path: PathBuf,
        site_path: PathBuf,
    },
}

impl Io {
    fn new(
        site_url: String,
        generator_bucket: String,
        s3_client: Option<aws_sdk_s3::Client>,
    ) -> Self {
        match s3_client {
            Some(s3_client) => Self::S3 {
                s3_client,
                generator_bucket,
                site_bucket: site_url,
            },
            None => Self::LocalFile {
                generator_path: Path::new("buckets").join(generator_bucket),
                site_path: Path::new("buckets").join(site_url),
            },
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

            Io::LocalFile { generator_path, .. } => {
                let path = generator_path.join(target);
                debug!("Reading {path:?}");
                let res = fs::read_to_string(&path).await;
                debug!(
                    "{} {path:?}",
                    if res.is_ok() {
                        "Read"
                    } else {
                        "Failed to read"
                    }
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

            Io::LocalFile { site_path, .. } => {
                let path = site_path.join(target);
                debug!("Writing to {path:?}");
                let res = fs::write(&path, content).await;
                debug!(
                    "{} {path:?}",
                    if res.is_ok() {
                        "Wrote"
                    } else {
                        "Failed to write"
                    }
                );
                res?
            }
        }

        Ok(())
    }
}

async fn read_template(io: &Io) -> Result<String> {
    io.read(SITE_INDEX_TEMPLATE).await
}

async fn read_list(io: &Io, site_url: &str) -> Result<ListOfLists> {
    let content = io.read(&format!("{site_url}.json")).await?;
    let list_of_lists: ListOfLists = serde_json::from_str(content.as_str())?;
    trace!("{list_of_lists:?}");

    list_of_lists.validate()
}

fn div_id_safe(_: &State, value: String) -> Result<String, Error> {
    Ok(inner_div_id_safe(value))
}

fn inner_div_id_safe<S>(value: S) -> String
where
    S: Into<String>,
{
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[^[_0-9A-Za-z]]").unwrap());

    RE.replace_all(&value.into().replace(' ', "_"), "")
        .into_owned()
}

pub async fn update_site(
    site_url: String,
    generator_bucket: String,
    s3_client: Option<aws_sdk_s3::Client>,
    minify: bool,
) -> Result<()> {
    let io = Io::new(site_url.clone(), generator_bucket, s3_client);

    let (template, list_of_lists) =
        tokio::try_join!(read_template(&io), read_list(&io, &site_url),)?;

    let mut env = Environment::new();
    env.add_template(SITE_INDEX, &template)?;
    env.add_filter(DIV_ID_SAFE, div_id_safe);

    let template = env.get_template(SITE_INDEX)?;

    debug!("Rendering {SITE_INDEX}");
    let site = template.render(&list_of_lists)?;
    debug!("Rendered {SITE_INDEX}");

    let site = if minify {
        let original_size = site.len();
        debug!("Minifying {SITE_INDEX} (original size: {original_size})",);

        let site = minify_html::minify(site.as_bytes(), &Cfg::new());

        debug!(
            "Minified {SITE_INDEX}: {:.1}% (new size: {})",
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
