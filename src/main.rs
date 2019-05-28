#[macro_use]
extern crate failure;

use clap::{crate_authors, crate_description, crate_name, crate_version};
use clap::{App, AppSettings, Arg, SubCommand};
use dotenv::dotenv;
use failure::Error;
use std::path::Path;

const UPDATE_CMD: &str = "update";
const SOURCE_ARG: &str = "SOURCE";
const DEST_ARG: &str = "DESTINATION";

fn main() -> Result<(), Error> {
    dotenv().ok();
    env_logger::init();

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .setting(AppSettings::SubcommandRequired)
        .subcommand(
            SubCommand::with_name(UPDATE_CMD)
                .about(
                    "Runs the backup and updates older files in the destination",
                )
                .arg(
                    Arg::with_name(SOURCE_ARG)
                        .help("Sets the path of the source")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name(DEST_ARG)
                        .help("Sets the path of the destination")
                        .required(true)
                        .index(2),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        (UPDATE_CMD, Some(arg_matches)) => {
            let source = arg_matches
                .value_of(SOURCE_ARG)
                .expect(&format!("{} must be provided", SOURCE_ARG));
            let dest = arg_matches
                .value_of(DEST_ARG)
                .expect(&format!("{} must be provided", DEST_ARG));
            bkup::run(&Path::new(source), &Path::new(dest))
        }
        _ => Err(format_err!("{}", matches.usage())),
    }
}
