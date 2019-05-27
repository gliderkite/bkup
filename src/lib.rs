#[macro_use]
extern crate failure;

use failure::{err_msg, Error};
use log::{info, trace};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Enumerates the possible results of a directory comparison.
#[derive(Debug, PartialEq)]
pub enum DirCmp {
    Same,
    Different,
}

/// Enumerates the possible results of a file comparison.
#[derive(Debug, PartialEq)]
pub enum FileCmp {
    Same,
    Older,
    Newer,
}

/// Represents the delta between the directory entry it points to and the
/// directory entry it has been compared to.
#[derive(Debug)]
pub struct DirDelta<'a> {
    entry: &'a DirEntry, // directory entry used for the comparison
    diff: DirCmp, // comparison result between the directory entry and the other
    entries: HashMap<&'a Path, EntryCmp<'a>>, // comparison results for each sub-entry
}

impl<'a> DirDelta<'a> {
    /*
    /// Creates a new directory difference from the given entry.
    fn new(entry: &'a DirEntry) -> Self {
        DirDelta {
            entry,
            delta: None,
            entries: HashMap::new(),
        }
    }
    */

    /// Creates a new directory difference from the given entries to compare.
    fn new(entry: &'a DirEntry, other: &'a DirEntry) -> Result<Self, Error> {
        let mut delta = DirDelta {
            entry,
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
pub struct FileDelta<'a> {
    entry: &'a FileEntry, // file entry used for the comparison
    diff: FileCmp,        // comparison result
}

impl<'a> FileDelta<'a> {
    /// Creates a new file delta from the given entries.
    fn new(entry: &'a FileEntry, other: &'a FileEntry) -> Result<Self, Error> {
        let diff = entry.cmp(other)?;
        Ok(FileDelta { entry, diff })
    }
}

#[derive(Debug)]
pub enum EntryCmp<'a> {
    Dir(DirDelta<'a>),
    File(FileDelta<'a>),
    NotFound,
}

#[derive(Debug)]
pub struct DirEntry {
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
                // the entry doesn't exist in the second directory
                Ok(EntryCmp::NotFound)
            };
            info!("Diff {:?} => {:?}", e1, cmp_res);
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
pub struct FileEntry {
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

    /// Compares self with another file entry.
    fn cmp(&self, other: &FileEntry) -> Result<FileCmp, Error> {
        let path1 = self.path.as_path();
        let path2 = other.path.as_path();
        let name1 = path1.file_name();
        let name2 = path2.file_name();
        // check filenames
        match (name1, name2) {
            (Some(name1), Some(name2)) if name1 == name2 => {
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
pub enum Entry {
    // Directory
    Dir(DirEntry),
    // File
    File(FileEntry),
}

impl Entry {
    /// Creates a new entry that represents a directory.
    fn new_dir(path: &Path) -> Result<Entry, Error> {
        Ok(Entry::Dir(DirEntry::new(path)?))
    }

    /// Creates a new entry that represents a file.
    fn new_file(path: &Path) -> Result<Entry, Error> {
        Ok(Entry::File(FileEntry::new(path)?))
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
                        .and_then(|p| p.to_str())
                        .map(|s| PathBuf::from(s))
                        .ok_or(format_err!(
                            "Cannot get the filename for '{:?}'!",
                            path
                        ))?;

                    if path.is_dir() {
                        trace!("New sub-directory: {:?}", path);
                        let mut dir = Entry::new_dir(&path)?;
                        // dfs with recursion
                        dir.visit()?;
                        directory.entries.insert(file_name, dir);
                    } else if path.is_file() {
                        trace!("New file: {:?}", path);
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
    pub fn cmp<'a>(&'a self, other: &'a Entry) -> Result<EntryCmp<'a>, Error> {
        trace!("Comparing: {} - {}", self, other);
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
pub fn run() -> Result<(), Error> {
    let left = Path::new("./A");
    info!("Visiting directory {:?}", left);
    let left = Entry::visit_dir(left)?;

    let right = Path::new("./B");
    info!("Visiting directory {:?}", right);
    let right = Entry::visit_dir(right)?;

    let cmp_res = left.cmp(&right)?;
    info!("Diff comparison: {:?}", cmp_res);

    Ok(())
}
