use lambda_runtime::{service_fn, LambdaEvent};
use list_of_lists::generator;
use serde_json::{json, Value};
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

async fn function(_: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    list_of_lists::set_up_logger(module_path!(), false)?;

    let site_name = env::var(list_of_lists::SITE_NAME_VAR)?;
    let site_url = env::var(list_of_lists::SITE_URL_VAR)?;

    generator::update_site(site_name, site_url, USE_S3, MINIFY).await?;

    Ok(json!({}))
}
