use aws_lambda_events::s3::S3Event;
use lambda_runtime::{LambdaEvent, service_fn};
use list_of_lists::generator;
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
    list_of_lists::set_up_logger(module_path!(), false)?;

    let site_name = env::var(list_of_lists::SITE_NAME_VAR)?;
    let site_url = env::var(list_of_lists::SITE_URL_VAR)?;

    let event: S3Event = serde_json::from_value(event.payload)?;
    if event.records.len() == 1 {
        for record in event.records {
            let bucket = record.s3.bucket.name;
            let key = record.s3.object.key;
            if let (Some(bucket), Some(key)) = (bucket, key) {
                info!("Updating {site_name} on update of {bucket}/{key}");
            }
        }
    } else {
        info!("Updating {site_name} on multiple updates");
    }

    generator::update_site(site_name, site_url, USE_S3, MINIFY).await?;

    Ok(json!({}))
}
