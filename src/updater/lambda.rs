use lambda_runtime::{handler_fn, Context};
use list_of_lists::{
    common::{self, LambdaError},
    updater,
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
    let dropbox_key = env::var(common::DB_KEY_VAR)?;
    let dropbox_path = env::var(common::DB_PATH_VAR)?;

    updater::try_update_list_file(site_name, site_url, dropbox_key, dropbox_path, false).await?;

    Ok(event)
}
