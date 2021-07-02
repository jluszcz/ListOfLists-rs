use lambda_runtime::{handler_fn, Context};
use list_of_lists::{
    common::{self, LambdaError},
    generator,
};
use serde_json::Value;
use std::env;

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    let func = handler_fn(function);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn function(event: Value, _: Context) -> Result<Value, LambdaError> {
    common::set_up_logger(false)?;

    let site_name = env::var(common::SITE_NAME_VAR)?;
    let site_url = env::var(common::SITE_URL_VAR)?;

    generator::update_site(site_name, site_url, true).await?;

    Ok(event)
}
