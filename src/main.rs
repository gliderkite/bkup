use dotenv::dotenv;
use failure::Error;
use log::error;
use std::process;

fn main() -> Result<(), Error> {
    dotenv().ok();
    env_logger::init();

    if let Err(err) = bkup::run() {
        error!("Application error: {}", err);
        process::exit(1);
    };

    Ok(())
}
