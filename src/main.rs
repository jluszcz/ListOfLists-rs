use anyhow::Result;
use aws_config::ConfigLoader;
use clap::Parser;
use jluszcz_rust_utils::set_up_logger;
use list_of_lists::{APP_NAME, generator};
use log::debug;

#[derive(Debug, Parser)]
#[command(
    name = "ListOfLists-Generator",
    version,
    author,
    infer_long_args = true
)]
struct Args {
    /// Site URL, e.g. 'foo.list'.
    #[arg(short = 'u', long, env = list_of_lists::SITE_URL_VAR)]
    site_url: String,

    /// Generator bucket name. Defaults to 'generator' for local use.
    #[arg(
        short = 'g',
        long,
        default_value = "generator",
        env = list_of_lists::GENERATOR_BUCKET_VAR
    )]
    generator_bucket: String,

    /// Verbose mode. Use -v for DEBUG, -vv for TRACE level logging.
    #[arg(short = 'v', action = clap::ArgAction::Count)]
    verbosity: u8,

    /// If provided, use S3 rather than local files.
    #[arg(short = 'r', long = "remote")]
    use_s3: bool,

    /// Minify generated site.
    #[arg(short = 'm', long)]
    minify: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    set_up_logger(APP_NAME, module_path!(), args.verbosity)?;
    debug!("Args: {args:?}");

    let s3_client = if args.use_s3 {
        let aws_config = ConfigLoader::default().load().await;
        Some(aws_sdk_s3::Client::new(&aws_config))
    } else {
        None
    };

    generator::update_site(args.site_url, args.generator_bucket, s3_client, args.minify).await
}
