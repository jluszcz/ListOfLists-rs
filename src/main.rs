use anyhow::Result;
use clap::{Arg, Command};
use list_of_lists::generator;
use log::debug;

#[derive(Debug)]
struct Args {
    site_name: String,
    site_url: String,
    verbose: bool,
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
                .takes_value(true)
                .env(list_of_lists::SITE_NAME_VAR)
                .help("Site name, e.g. foolist."),
        )
        .arg(
            Arg::new("site-url")
                .short('u')
                .long("site-url")
                .required(true)
                .takes_value(true)
                .env(list_of_lists::SITE_URL_VAR)
                .help("Site URL, e.g. 'foo.list'."),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Verbose mode. Outputs DEBUG and higher log messages."),
        )
        .arg(
            Arg::new("local")
                .short('l')
                .long("local")
                .help("If provided, use local files rather than S3."),
        )
        .arg(
            Arg::new("minify")
                .short('m')
                .long("minify")
                .help("Minify generated site."),
        )
        .get_matches();

    let site_name = matches.value_of("site-name").map(|l| l.into()).unwrap();

    let site_url = matches.value_of("site-url").map(|l| l.into()).unwrap();

    let verbose = matches.is_present("verbose");

    let use_s3 = !matches.is_present("local");

    let minify = matches.is_present("minify");

    Args {
        site_name,
        site_url,
        verbose,
        use_s3,
        minify,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();
    list_of_lists::set_up_logger(module_path!(), args.verbose)?;
    debug!("{:?}", args);

    generator::update_site(args.site_name, args.site_url, args.use_s3, args.minify).await
}
