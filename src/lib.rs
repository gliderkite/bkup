#[macro_use]
extern crate failure;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod entry;

use entry::Entry;
use failure::Error;
use log::*;
use std::{path::PathBuf, thread, time::Duration};

/// Updates the destination directory according to its delta with the source
/// directory.
pub fn update(
    source: PathBuf,
    dest: PathBuf,
    accuracy: Duration,
    ignore: bool,
) -> Result<(), Error> {
    info!(
        "Updating directory {:?} with content of {:?} ({:?} accuracy - ignore: {})",
        dest, source, accuracy, ignore
    );

    // spawn thread used to visit the destination directory
    let handle = thread::spawn(move || {
        info!("Exploring destination directory {:?}", dest);
        Entry::directory(&dest, ignore)
    });

    info!("Exploring source directory {:?}", source);
    let source = Entry::directory(&source, ignore)?;

    let dest = handle
        .join()
        .expect("Couldn't join on the destination visit thread")?;

    info!("Computing difference");
    let delta = source.cmp(&dest, &accuracy)?;
    debug!("Delta: {:?}", delta);

    if let Some(delta) = delta {
        info!("Updating destination");
        delta.clear()?;
    }

    info!("Update completed");
    Ok(())
}
