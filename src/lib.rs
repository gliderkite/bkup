#[macro_use]
extern crate failure;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

use failure::{err_msg, Error};
use fs_extra::dir;
use log::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Enumerates the possible results of a directory comparison.
#[derive(Debug, PartialEq)]
enum DirCmp {
    Same,
    Different,
}

/// Enumerates the possible results of a file comparison.
#[derive(Debug, PartialEq)]
enum FileCmp {
    Same,
    Older,
    Newer,
}

/// Represents the delta between the directory entry it points to and the
/// directory entry it has been compared to.
#[derive(Debug)]
struct DirDelta<'a> {
    entry: &'a DirEntry, // source directory entry used for the comparison
    other: &'a DirEntry, // destination directory entry used for the comparison
    diff: DirCmp, // comparison result between the directory entry and the other
    entries: HashMap<&'a Path, EntryCmp<'a>>, // comparison results for each sub-entry
}

impl<'a> DirDelta<'a> {
    /// Creates a new directory difference from the given entries to compare.
    fn new(entry: &'a DirEntry, other: &'a DirEntry) -> Result<Self, Error> {
        let mut delta = DirDelta {
            entry,
            other,
            diff: DirCmp::Same,
            entries: HashMap::new(),
        };
        delta.diff = entry.cmp(other, &mut delta)?;
        Ok(delta)
    }
}

/// Represents the delta between the file entry it points to and the file entry
/// it has been compared to.
#[derive(Debug)]
struct FileDelta<'a> {
    entry: &'a FileEntry, // source file entry used for the comparison
    other: &'a FileEntry, // destination file entry used for the comparison
    diff: FileCmp,        // comparison result
}

impl<'a> FileDelta<'a> {
    /// Creates a new file delta from the given entries.
    fn new(entry: &'a FileEntry, other: &'a FileEntry) -> Result<Self, Error> {
        let diff = entry.cmp(other)?;
        Ok(FileDelta { entry, other, diff })
    }
}

#[derive(Debug)]
enum EntryCmp<'a> {
    Dir(DirDelta<'a>),
    File(FileDelta<'a>),
    NotFound { entry: &'a Entry, other: PathBuf }, // `entry` not found on the `other` path
}

#[derive(Debug)]
struct DirEntry {
    // directory path
    path: PathBuf,
    // sub-entries where the key is the file name
    entries: HashMap<PathBuf, Entry>,
}

impl DirEntry {
    /// Creates a new directory entry.
    fn new(path: &Path) -> Result<DirEntry, Error> {
        if path.is_dir() {
            Ok(DirEntry {
                path: path.to_path_buf(),
                entries: HashMap::new(),
            })
        } else {
            Err(format_err!(
                "The given directory '{:?}' does not exist!",
                path
            ))
        }
    }

    /// Copies self into the given destination.
    fn copy(&self, dest: &Path) -> Result<DirEntry, Error> {
        info!("Copy directory {:?} to {:?}", self.path, dest);
        fs::create_dir(dest)?;
        let parent = dest
            .parent()
            .ok_or(format_err!("Cannot get parent of {:?}", dest))?;
        dir::copy(&self.path, parent, &dir::CopyOptions::new())?;
        DirEntry::new(dest)
    }

    /// Compares self with another directory entry and store the difference in
    /// the given delta data structure.
    fn cmp<'a>(
        &'a self,
        other: &'a DirEntry,
        delta: &mut DirDelta<'a>,
    ) -> Result<DirCmp, Error> {
        // true only if all the entries of self and other are the same
        let mut is_same = true;
        // compare each entry of the first directory with the content of
        // the second directory
        for (name, e1) in &self.entries {
            let cmp_res = if let Some(e2) = other.entries.get(name) {
                e1.cmp(e2)
            } else {
                let dest_path: PathBuf =
                    [other.path.as_path(), e1.file_name()?].iter().collect();
                // the entry doesn't exist in the second directory
                Ok(EntryCmp::NotFound {
                    entry: e1,
                    other: dest_path,
                })
            };
            debug!("Difference for {:?}: {:?}", e1, cmp_res);
            let cmp_res = cmp_res?;

            // check if all the entries are the same by finding the first difference
            if is_same {
                is_same = match &cmp_res {
                    EntryCmp::Dir(dir) => dir.diff == DirCmp::Same,
                    EntryCmp::File(file) => file.diff == FileCmp::Same,
                    _ => false,
                };
            }

            delta.entries.insert(name, cmp_res);
        }

        if is_same {
            Ok(DirCmp::Same)
        } else {
            Ok(DirCmp::Different)
        }
    }
}

#[derive(Debug)]
struct FileEntry {
    // file path
    path: PathBuf,
}

impl FileEntry {
    /// Creates a new file entry.
    fn new(path: &Path) -> Result<FileEntry, Error> {
        if path.is_file() {
            Ok(FileEntry {
                path: path.to_path_buf(),
            })
        } else {
            Err(format_err!("The given file '{:?}' does not exist!", path))
        }
    }

    /// Copies self into the given destination.
    fn copy(&self, dest: &Path) -> Result<FileEntry, Error> {
        info!("Copy file {:?} to {:?}", self.path, dest);
        fs::copy(&self.path, dest)?;
        FileEntry::new(dest)
    }

    /// Compares self with another file entry.
    fn cmp(&self, other: &FileEntry) -> Result<FileCmp, Error> {
        let path1 = self.path.as_path();
        let path2 = other.path.as_path();
        let name1 = path1.file_name();
        trace!("Filename: {:?}", name1);
        let name2 = path2.file_name();
        trace!("Filename: {:?}", name2);
        // check filenames
        match (name1, name2) {
            (Some(name1), Some(name2)) => {
                if name1 != name2 {
                    warn!("Comparing files with different file names");
                }
                // check modification time
                let t1 = fs::metadata(path1)?.modified()?;
                let t2 = fs::metadata(path2)?.modified()?;
                match t1.cmp(&t2) {
                    Ordering::Less => Ok(FileCmp::Older),
                    Ordering::Greater => Ok(FileCmp::Newer),
                    Ordering::Equal => Ok(FileCmp::Same),
                }
            }
            _ => Err(format_err!(
                "Invalid filenames for {:?} {:?}!",
                path1,
                path2
            )),
        }
    }
}

#[derive(Debug)]
enum Entry {
    // Directory
    Dir(DirEntry),
    // File
    File(FileEntry),
}

impl Entry {
    /// Creates a new entry that represents a directory.
    fn new_dir(path: &Path) -> Result<Entry, Error> {
        Entry::new_dir_entry(DirEntry::new(path)?)
    }

    /// Creates a new entry that represents a directory.
    fn new_dir_entry(entry: DirEntry) -> Result<Entry, Error> {
        Ok(Entry::Dir(entry))
    }

    /// Creates a new entry that represents a file.
    fn new_file(path: &Path) -> Result<Entry, Error> {
        Entry::new_file_entry(FileEntry::new(path)?)
    }

    /// Creates a new entry that represents a file.
    fn new_file_entry(entry: FileEntry) -> Result<Entry, Error> {
        Ok(Entry::File(entry))
    }

    /// Creates a new entry that represents a directory and populates its
    /// entries by visiting it.
    fn visit_dir(path: &Path) -> Result<Entry, Error> {
        Entry::new_dir(path).and_then(|mut d| {
            d.visit()?;
            Ok(d)
        })
    }

    /// Gets the path of the entry.
    fn path(&self) -> &Path {
        match self {
            Entry::Dir(e) => &e.path,
            Entry::File(e) => &e.path,
        }
    }

    /// Gets the filename of the entry.
    fn file_name(&self) -> Result<&Path, Error> {
        self.path()
            .file_name()
            .map(|s| Path::new(s))
            .ok_or(format_err!("Cannot get the filename for '{}'", self))
    }

    /// Copies self into the given destination.
    fn copy(&self, dest: &Path) -> Result<Entry, Error> {
        match self {
            Entry::Dir(e) => Entry::new_dir_entry(e.copy(dest)?),
            Entry::File(e) => Entry::new_file_entry(e.copy(dest)?),
        }
    }

    /// Visit and populate the Entry.
    /// Only entries that represent directories can be visited.
    fn visit(&mut self) -> Result<(), Error> {
        // check that the entry is a directory
        match self {
            Entry::Dir(directory) => {
                // iterate over the directory entries
                for e in fs::read_dir(&directory.path)? {
                    let e = e?;
                    let path = e.path();

                    // get the entry filename if any
                    let file_name = path
                        .file_name()
                        .map(|s| PathBuf::from(s))
                        .ok_or(format_err!(
                            "Cannot get the filename for '{:?}'",
                            path
                        ))?;

                    if path.is_dir() {
                        debug!("New sub-directory: {:?}", path);
                        let mut dir = Entry::new_dir(&path)?;
                        // dfs with recursion
                        dir.visit()?;
                        directory.entries.insert(file_name, dir);
                    } else if path.is_file() {
                        debug!("New file: {:?}", path);
                        directory
                            .entries
                            .insert(file_name, Entry::new_file(&path)?);
                    }
                }
                Ok(())
            }
            _ => Err(err_msg("Cannot visit a file!")),
        }
    }

    /// Compares self with another entry.
    fn cmp<'a>(&'a self, other: &'a Entry) -> Result<EntryCmp<'a>, Error> {
        debug!("Comparing: {} to {}", self, other);
        match (self, other) {
            (Entry::Dir(dir1), Entry::Dir(dir2)) => {
                let delta = DirDelta::new(dir1, dir2)?;
                Ok(EntryCmp::Dir(delta))
            }
            (Entry::File(f1), Entry::File(f2)) => {
                let delta = FileDelta::new(f1, f2)?;
                Ok(EntryCmp::File(delta))
            }
            _ => Err(err_msg("Cannot compare different type of entries!")),
        }
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.path().display())
    }
}

/// Runs the directories comparison.
pub fn run(source: &Path, dest: &Path) -> Result<(), Error> {
    info!("Exploring directory {:?}", source);
    let source = Entry::visit_dir(source)?;

    info!("Exploring directory {:?}", dest);
    let dest = Entry::visit_dir(dest)?;

    info!("Computing difference");
    let diff = source.cmp(&dest)?;
    debug!("Difference: {:?}", diff);

    info!("Updating destination");
    update(&diff)?;
    info!("Update completed");

    Ok(())
}

/// Runs the update according to the given comparison result.
fn update<'a>(diff: &EntryCmp<'a>) -> Result<(), Error> {
    match diff {
        EntryCmp::Dir(delta) => {
            debug!("Directory delta: {:?}", delta);
            if delta.diff == DirCmp::Different {
                for (_, entry) in &delta.entries {
                    update(entry)?;
                }
            }
        }
        EntryCmp::File(delta) => {
            debug!("File delta: {:?}", delta);
            if delta.diff == FileCmp::Newer {
                delta.entry.copy(&delta.other.path)?;
            }
        }
        EntryCmp::NotFound { entry, other } => {
            debug!("Not found: {:?} in {:?}", entry, other);
            entry.copy(other)?;
        }
    };
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::env;
    use std::{thread, time};
    use uuid::Uuid;

    lazy_static! {
        /// Interval used to write files with significant difference on the
        /// modification time stored in the metadata.
        static ref SLEEP_INTERVAL: time::Duration = time::Duration::from_millis(10);
    }

    #[test]
    fn test_cmp_files() {
        let temp_dir = env::temp_dir();
        // create older file
        let older = Uuid::new_v4().to_simple().to_string();
        let older: PathBuf =
            [temp_dir.as_path(), Path::new(&older)].iter().collect();
        fs::write(&older, "").expect("Cannot write older file");
        thread::sleep(*SLEEP_INTERVAL);
        // create newer file
        let newer = Uuid::new_v4().to_simple().to_string();
        let newer: PathBuf =
            [temp_dir.as_path(), Path::new(&newer)].iter().collect();
        assert_ne!(older, newer);
        fs::write(&newer, "").expect("Cannot write newer file");

        // create entries
        let older =
            FileEntry::new(older.as_path()).expect("Cannot create older entry");
        let newer =
            FileEntry::new(newer.as_path()).expect("Cannot create newer entry");
        // compare entries
        let cmp = older.cmp(&newer).expect("Cannot compare entries");
        assert_eq!(cmp, FileCmp::Older);
        let cmp = older.cmp(&older).expect("Cannot compare entries");
        assert_eq!(cmp, FileCmp::Same);
        let cmp = newer.cmp(&older).expect("Cannot compare entries");
        assert_eq!(cmp, FileCmp::Newer);
        let cmp = newer.cmp(&newer).expect("Cannot compare entries");
        assert_eq!(cmp, FileCmp::Same);

        // create a copy of the older file
        let copy = older
            .copy(newer.path.as_path())
            .expect("Cannot create a copy");
        let cmp = older.cmp(&copy).expect("Cannot compare entries");
        assert_eq!(cmp, FileCmp::Older);
        let cmp = copy.cmp(&older).expect("Cannot compare entries");
        assert_eq!(cmp, FileCmp::Newer);
    }

}
