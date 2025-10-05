use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use jluszcz_rust_utils::{Verbosity, set_up_logger};
use list_of_lists::{APP_NAME, generator};
use log::debug;

#[derive(Debug)]
struct Args {
    site_name: String,
    site_url: String,
    verbosity: Verbosity,
    use_s3: bool,
    minify: bool,
}

fn parse_args() -> Args {
    let matches = Command::new("ListOfLists-Generator")
        .version("0.1")
        .author("Jacob Luszcz")
        .arg(
            Arg::new("site-name")
                .short('s')
                .long("site-name")
                .required(true)
                .env(list_of_lists::SITE_NAME_VAR)
                .help("Site name, e.g. foolist."),
        )
        .arg(
            Arg::new("site-url")
                .short('u')
                .long("site-url")
                .required(true)
                .env(list_of_lists::SITE_URL_VAR)
                .help("Site URL, e.g. 'foo.list'."),
        )
        .arg(
            Arg::new("verbosity")
                .short('v')
                .action(ArgAction::Count)
                .help("Verbose mode. Use -v for DEBUG, -vv for TRACE level logging."),
        )
        .arg(
            Arg::new("remote")
                .short('r')
                .long("remote")
                .action(ArgAction::SetTrue)
                .help("If provided, use S3 rather than local files."),
        )
        .arg(
            Arg::new("minify")
                .short('m')
                .long("minify")
                .action(ArgAction::SetTrue)
                .help("Minify generated site."),
        )
        .get_matches();

    let site_name = matches
        .get_one::<String>("site-name")
        .map(|l| l.into())
        .unwrap();

    let site_url = matches
        .get_one::<String>("site-url")
        .map(|l| l.into())
        .unwrap();

    let verbosity = matches.get_count("verbosity").into();

    let use_s3 = matches.get_flag("remote");

    let minify = matches.get_flag("minify");

    Args {
        site_name,
        site_url,
        verbosity,
        use_s3,
        minify,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();
    set_up_logger(APP_NAME, module_path!(), args.verbosity)?;
    debug!("Args: {args:?}");

    generator::update_site(args.site_name, args.site_url, args.use_s3, args.minify).await
}
