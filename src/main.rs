#[macro_use]
extern crate clap;

use clap::{App, ArgMatches};
use dotenv::dotenv;
use failure::{err_msg, Error};
use std::path::Path;

/// CLI commands
const UPDATE_CMD: &str = "update";

// CLI commands args
const DEST_ARG: &str = "dest";
const SOURCE_ARG: &str = "source";

fn main() -> Result<(), Error> {
    dotenv().ok();
    env_logger::init();

    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    match matches.subcommand() {
        (UPDATE_CMD, Some(matches)) => cmd::update(matches),
        _ => Err(err_msg("Invalid command")),
    }
}

mod cmd {
    use super::*;

    /// Runs the update command.
    pub fn update(matches: &ArgMatches) -> Result<(), Error> {
        let source = matches
            .value_of(SOURCE_ARG)
            .expect(&format!("'{}' must be provided", SOURCE_ARG));
        let dest = matches
            .value_of(DEST_ARG)
            .expect(&format!("'{}' must be provided", DEST_ARG));
        bkup::update(&Path::new(source), &Path::new(dest))
    }
}
