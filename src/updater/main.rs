use anyhow::Result;
use clap::{App, Arg};
use list_of_lists::{common, updater};
use log::debug;

#[derive(Debug)]
struct Args {
    verbose: bool,
    force: bool,
    site_name: String,
    site_url: String,
    dropbox_key: String,
    dropbox_path: String,
}

fn parse_args() -> Args {
    let matches = App::new("ListOfLists-Updater")
        .version("0.1")
        .author("Jacob Luszcz")
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Verbose mode. Outputs DEBUG and higher log messages."),
        )
        .arg(
            Arg::with_name("force")
                .short("f")
                .long("force")
                .help("Force an update to S3 even if the list is already up to date."),
        )
        .arg(
            Arg::with_name("site-name")
                .short("s")
                .long("site-name")
                .required(true)
                .takes_value(true)
                .env(common::SITE_NAME_VAR)
                .help("Site name, e.g. foolist."),
        )
        .arg(
            Arg::with_name("site-url")
                .short("u")
                .long("site-url")
                .required(true)
                .takes_value(true)
                .env(common::SITE_URL_VAR)
                .help("Site URL, e.g. 'foo.list'."),
        )
        .arg(
            Arg::with_name("dropbox-key")
                .short("k")
                .long("db-key")
                .required(true)
                .takes_value(true)
                .env(common::DB_KEY_VAR)
                .hide_env_values(true)
                .help("Access key used to access Dropbox."),
        )
        .arg(
            Arg::with_name("dropbox-path")
                .short("p")
                .long("db-path")
                .required(true)
                .takes_value(true)
                .env(common::DB_PATH_VAR)
                .hide_env_values(true)
                .help("Path of list file within Dropbox."),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");

    let force = matches.is_present("force");

    let site_name = matches.value_of("site-name").map(|l| l.into()).unwrap();

    let site_url = matches.value_of("site-url").map(|l| l.into()).unwrap();

    let dropbox_key = matches.value_of("dropbox-key").map(|l| l.into()).unwrap();

    let dropbox_path = matches.value_of("dropbox-path").map(|l| l.into()).unwrap();

    Args {
        verbose,
        force,
        site_name,
        site_url,
        dropbox_key,
        dropbox_path,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();
    common::set_up_logger(args.verbose)?;
    debug!("{:?}", args);

    updater::try_update_list_file(
        args.site_name,
        args.site_url,
        args.dropbox_key,
        args.dropbox_path,
        args.force,
    )
    .await
}
