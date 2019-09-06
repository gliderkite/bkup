#[macro_use]
extern crate clap;

use clap::{App, ArgMatches};
use dotenv::dotenv;
use failure::{err_msg, Error};
use std::env;
use std::path::PathBuf;
use std::time::Duration;

/// CLI commands
const UPDATE_CMD: &str = "update";
// CLI commands args
const ACCURACY_ARG: &str = "accuracy";
const DEST_ARG: &str = "dest";
const IGNORE_ARG: &str = "ignore";
const SOURCE_ARG: &str = "source";

// Default accuracy in ms (2s for FAT filesystem as worst case scenario)
const DEFAULT_ACCURACY: &str = "2000";

fn main() -> Result<(), Error> {
    // set default value for logger priority to INFO if not set
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "bkup=info");
    }

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
        let accuracy = matches
            .value_of(ACCURACY_ARG)
            .unwrap_or(DEFAULT_ACCURACY)
            .parse::<u64>()
            .map(|a| Duration::from_millis(a))
            .expect("Accuracy must be a valid u64");
        let ignore = matches.is_present(IGNORE_ARG);
        bkup::update(
            PathBuf::from(source),
            PathBuf::from(dest),
            accuracy,
            ignore,
        )
    }
}
