#[macro_use]
extern crate failure;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod entries;

use entries::{Entry, EntryDelta};
use failure::Error;
use log::*;
use std::path::Path;

/// Runs the directories comparison.
pub fn run(source: &Path, dest: &Path) -> Result<(), Error> {
    info!("Exploring directory {:?}", source);
    let source = Entry::new_dir(source)?;

    info!("Exploring directory {:?}", dest);
    let dest = Entry::new_dir(dest)?;

    info!("Computing difference");
    let diff = source.cmp(&dest)?;
    debug!("Difference: {:?}", diff);

    info!("Updating destination");
    update(&diff)?;
    info!("Update completed");

    Ok(())
}

/// Runs the update according to the given comparison result.
fn update<'a>(diff: &EntryDelta<'a>) -> Result<(), Error> {
    match diff {
        EntryDelta::Dir(delta) => {
            debug!("Directory delta: {:?}", delta);
            if !delta.is_none() {
                for entry in delta.entries() {
                    update(entry)?;
                }
            }
        }
        EntryDelta::File(delta) => {
            debug!("File delta: {:?}", delta);
            if delta.is_newer() {
                delta.source().copy(&delta.destination().path())?;
            }
        }
        EntryDelta::NotFound { entry, path } => {
            debug!("Not found: {:?} in {:?}", entry, path);
            entry.copy(path)?;
        }
    };
    Ok(())
}
