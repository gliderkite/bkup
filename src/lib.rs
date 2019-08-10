#[macro_use]
extern crate failure;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod entry;

use entry::Entry;
use failure::Error;
use ignore::gitignore::Gitignore;
use log::*;
use std::path::Path;
use std::time::Duration;

/// Updates the destination directory according to its delta with the source
/// directory.
pub fn update(
    source: &Path,
    dest: &Path,
    accuracy: Duration,
) -> Result<(), Error> {
    let (ignore, _) = Gitignore::new(".bkignore");

    info!("Exploring directory {:?}", source);
    let source = Entry::directory(source, Some(&ignore))?;
    info!("Exploring directory {:?}", dest);
    let dest = Entry::directory(dest, Some(&ignore))?;

    info!("Computing difference");
    let delta = source.cmp(&dest, &accuracy)?;
    debug!("Delta: {:?}", delta);
    info!("Updating destination");
    delta.clear()?;
    info!("Update completed");
    Ok(())
}
