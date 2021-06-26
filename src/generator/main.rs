use anyhow::Result;
use clap::{App, Arg};
use list_of_lists::{common, generator};
use log::debug;
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
            Arg::with_name("use-s3")
                .long("s3")
                .help("If provided, use S3 rather than local files."),
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

    let use_s3 = matches.is_present("use-s3");

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
async fn main() -> Result<()> {
    let args = parse_args();
    common::set_up_logger(args.verbose)?;
    debug!("{:?}", args);

    generator::update_site(args.site_name, args.site_url, args.use_s3).await
}
