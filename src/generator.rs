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
const OPTIMIZE_IMPORT: &str = "optimize_import";

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

fn optimize_import(_: &State, value: String) -> Result<String, Error> {
    Ok(inner_optimize_import(value))
}

fn inner_optimize_import<S>(value: S) -> String
where
    S: Into<String>,
{
    static SCRIPT_TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)<script\b([^>]*)>"#).unwrap());
    // Match defer/async only as attribute names — preceded by start-of-attrs or
    // whitespace, followed by whitespace, '=', '/', or end-of-attrs — so values
    // like src="https://example.com/defer.js" don't trip this.
    static DEFER_OR_ASYNC_ATTR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(?:^|\s)(?:defer|async)(?:\s|=|/|$)").unwrap());
    static LINK_TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)<link\b([^>]*)>"#).unwrap());
    static REL_STYLESHEET: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)\brel\s*=\s*["']?stylesheet["']?"#).unwrap());

    let value = SCRIPT_TAG
        .replace_all(&value.into(), |caps: &regex::Captures| {
            let attrs = &caps[1];
            if DEFER_OR_ASYNC_ATTR.is_match(attrs) {
                caps[0].to_string()
            } else {
                format!("<script defer{attrs}>")
            }
        })
        .into_owned();

    LINK_TAG
        .replace_all(&value, |caps: &regex::Captures| {
            let attrs = &caps[1];
            if !REL_STYLESHEET.is_match(attrs) {
                return caps[0].to_string();
            }
            let preload_attrs = REL_STYLESHEET.replace(attrs, r#"rel="preload" as="style""#);
            format!(
                r#"<link{preload_attrs} onload="this.onload=null;this.rel='stylesheet'"><noscript><link{attrs}></noscript>"#
            )
        })
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
    env.add_filter(OPTIMIZE_IMPORT, optimize_import);

    let template = env.get_template(SITE_INDEX)?;

    debug!("Rendering {SITE_INDEX}");
    let site = template.render(&list_of_lists)?;
    debug!("Rendered {SITE_INDEX}");

    let site = if minify {
        let original_size = site.len();
        debug!("Minifying {SITE_INDEX} (original size: {original_size})",);

        let mut cfg = Cfg::new();
        cfg.minify_css = true;
        cfg.minify_js = true;

        let site = minify_html::minify(site.as_bytes(), &cfg);

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

    #[test]
    fn test_optimize_import_adds_defer() {
        assert_eq!(
            r#"<script defer src="https://example.com/a.js"></script>"#,
            inner_optimize_import(r#"<script src="https://example.com/a.js"></script>"#),
        );
    }

    #[test]
    fn test_optimize_import_preserves_existing_defer() {
        let input = r#"<script src="https://example.com/a.js" defer></script>"#;
        assert_eq!(input, inner_optimize_import(input));
    }

    #[test]
    fn test_optimize_import_skips_async() {
        let input = r#"<script src="https://example.com/a.js" async></script>"#;
        assert_eq!(input, inner_optimize_import(input));
    }

    #[test]
    fn test_optimize_import_ignores_defer_in_attribute_value() {
        // 'defer' appears inside an attribute value, not as an attribute name.
        let input = r#"<script src="https://example.com/defer.js"></script>"#;
        let expected = r#"<script defer src="https://example.com/defer.js"></script>"#;
        assert_eq!(expected, inner_optimize_import(input));
    }

    #[test]
    fn test_optimize_import_handles_multiple_scripts() {
        let input = concat!(
            r#"<script src="https://example.com/a.js"></script>"#,
            r#"<script src="https://example.com/b.js" defer></script>"#,
        );
        let expected = concat!(
            r#"<script defer src="https://example.com/a.js"></script>"#,
            r#"<script src="https://example.com/b.js" defer></script>"#,
        );
        assert_eq!(expected, inner_optimize_import(input));
    }

    #[test]
    fn test_optimize_import_handles_multi_line_script() {
        let input = "<script\n    src=\"https://example.com/a.js\"\n    integrity=\"sha384-abc\"\n    crossorigin=\"anonymous\"></script>";
        let expected = "<script defer\n    src=\"https://example.com/a.js\"\n    integrity=\"sha384-abc\"\n    crossorigin=\"anonymous\"></script>";
        assert_eq!(expected, inner_optimize_import(input));
    }

    #[test]
    fn test_optimize_import_rewrites_stylesheet_link() {
        let input = r#"<link rel="stylesheet" href="https://example.com/a.css" integrity="sha384-abc" crossorigin="anonymous">"#;
        let expected = concat!(
            r#"<link rel="preload" as="style" href="https://example.com/a.css" integrity="sha384-abc" crossorigin="anonymous" onload="this.onload=null;this.rel='stylesheet'">"#,
            r#"<noscript><link rel="stylesheet" href="https://example.com/a.css" integrity="sha384-abc" crossorigin="anonymous"></noscript>"#,
        );
        assert_eq!(expected, inner_optimize_import(input));
    }

    #[test]
    fn test_optimize_import_leaves_non_stylesheet_link_unchanged() {
        let input = r#"<link rel="icon" href="favicon.ico">"#;
        assert_eq!(input, inner_optimize_import(input));
    }
}
