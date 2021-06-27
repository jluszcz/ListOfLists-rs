use anyhow::Result;
use clap::{App, Arg};
use lambda_runtime::{handler_fn, Context};
use list_of_lists::{
    common::{self, LambdaError},
    generator,
};
use log::debug;
use serde_json::Value;
use std::env;

#[derive(Debug)]
struct Args {
    verbose: bool,
    use_s3: bool,
    site_name: String,
    site_url: String,
}

fn parse_args() -> Args {
    let matches = App::new("ListOfLists-Generator")
        .version("0.1")
        .author("Jacob Luszcz")
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Verbose mode. Outputs DEBUG and higher log messages."),
        )
        .arg(
            Arg::with_name("local")
                .short("l")
                .long("local")
                .help("If provided, use local files rather than S3."),
        )
        .arg(
            Arg::with_name("site-name")
                .short("s")
                .long("site-name")
                .help("Site name, e.g. foolist."),
        )
        .arg(
            Arg::with_name("site-url")
                .short("u")
                .long("site-url")
                .help("Site URL, e.g. 'foo.list'."),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");

    let use_s3 = !matches.is_present("local");

    let site_name = matches
        .value_of("site-name")
        .map(|l| l.into())
        .or_else(|| env::var(common::SITE_NAME_VAR).ok())
        .expect("Missing site name");

    let site_url = matches
        .value_of("site-url")
        .map(|l| l.into())
        .or_else(|| env::var(common::SITE_URL_VAR).ok())
        .expect("Missing site URL");

    Args {
        verbose,
        use_s3,
        site_name,
        site_url,
    }
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    let func = handler_fn(function);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn function(event: Value, _: Context) -> Result<Value, LambdaError> {
    let args = parse_args();
    common::set_up_logger(args.verbose)?;
    debug!("{:?}", args);

    generator::update_site(args.site_name, args.site_url, args.use_s3).await?;

    Ok(event)
}
