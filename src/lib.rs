#[macro_use]
extern crate failure;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod entry;

use entry::Entry;
use failure::Error;
use log::*;
use std::path::Path;
use std::time::Duration;

/// Updates the destination directory according to its delta with the source
/// directory.
pub fn update(
    source: &Path,
    dest: &Path,
    accuracy: Duration,
    ignore: bool,
) -> Result<(), Error> {
    info!(
        "Updating directory {:?} with content of {:?} ({:?} accuracy - ignore: {})",
        dest, source, accuracy, ignore
    );

    info!("Exploring directory {:?}", source);
    let source = Entry::directory(source, ignore)?;
    info!("Exploring directory {:?}", dest);
    let dest = Entry::directory(dest, ignore)?;

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
