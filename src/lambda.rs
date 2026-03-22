use aws_config::ConfigLoader;
use aws_lambda_events::s3::S3Event;
use aws_sdk_s3::Client as S3Client;
use jluszcz_rust_utils::lambda;
use lambda_runtime::{LambdaEvent, service_fn};
use list_of_lists::{APP_NAME, generator, s3util};
use log::info;
use serde_json::{Value, json};
use std::env;

const MINIFY: bool = true;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let func = service_fn(function);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn function(event: LambdaEvent<Value>) -> Result<Value, lambda_runtime::Error> {
    lambda::init(APP_NAME, module_path!(), false).await?;

    let generator_bucket = env::var(list_of_lists::GENERATOR_BUCKET_VAR)?;

    let aws_config = ConfigLoader::default().load().await;
    let s3_client = S3Client::new(&aws_config);

    let event: S3Event = serde_json::from_value(event.payload)?;
    for record in event.records {
        let bucket = record.s3.bucket.name;
        let key = record.s3.object.key;
        if let (Some(bucket), Some(key)) = (bucket, key) {
            if key == generator::SITE_INDEX_TEMPLATE {
                info!("Regenerating all sites on update of {bucket}/{key}");
                let site_keys = s3util::list_keys(&s3_client, &generator_bucket, ".json").await?;
                for site_key in site_keys {
                    if let Some(site_url) = site_key.strip_suffix(".json") {
                        generator::update_site(
                            site_url.to_string(),
                            generator_bucket.clone(),
                            Some(s3_client.clone()),
                            MINIFY,
                        )
                        .await?;
                    }
                }
            } else if let Some(site_url) = key.strip_suffix(".json") {
                info!("Updating {site_url} on update of {bucket}/{key}");
                generator::update_site(
                    site_url.to_string(),
                    generator_bucket.clone(),
                    Some(s3_client.clone()),
                    MINIFY,
                )
                .await?;
            }
        }
    }

    Ok(json!({}))
}
