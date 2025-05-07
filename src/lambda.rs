use aws_lambda_events::s3::S3Event;
use lambda_runtime::{LambdaEvent, service_fn};
use lambda_utils::{emit_rustc_metric, set_up_logger};
use list_of_lists::{APP_NAME, generator};
use log::info;
use serde_json::{Value, json};
use std::env;
use std::error::Error;

type LambdaError = Box<dyn Error + Send + Sync + 'static>;

const USE_S3: bool = true;
const MINIFY: bool = true;

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    let func = service_fn(function);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn function(event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    set_up_logger(APP_NAME, module_path!(), false)?;
    emit_rustc_metric(APP_NAME).await;

    let site_name = env::var(list_of_lists::SITE_NAME_VAR)?;
    let site_url = env::var(list_of_lists::SITE_URL_VAR)?;

    let event: S3Event = serde_json::from_value(event.payload)?;
    for record in event.records {
        let bucket = record.s3.bucket.name;
        let key = record.s3.object.key;
        if let (Some(bucket), Some(key)) = (bucket, key) {
            info!("Updating {site_name} on update of {bucket}/{key}");
        }
    }

    generator::update_site(site_name, site_url, USE_S3, MINIFY).await?;

    Ok(json!({}))
}
