#[macro_use]
extern crate failure;

use dotenv::dotenv;
use failure::{err_msg, Error};
use log::{info, trace};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Error> {
    dotenv().ok();
    env_logger::init();

    let left = Path::new("./A");
    info!("Visiting directory {:?}", left);
    let left = visit_dir(left)?;

    let right = Path::new("./B");
    info!("Visiting directory {:?}", right);
    let right = visit_dir(right)?;

    let diff = left.cmp(&right)?;
    info!("Diff comparison: {:?}", diff);

    //info!("{:?}", paths);
    Ok(())
}

#[derive(Debug, PartialEq)]
enum DirCmp {
    Same,
    Different,
}

#[derive(Debug, PartialEq)]
enum FileCmp {
    Same,
    Older,
    Newer,
}

#[derive(Debug)]
enum EntryCmp {
    Dir(DirCmp),
    File(FileCmp),
    NotFound,
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
            Err(err_msg("The given directory does not exist!"))
        }
    }

    /// Compares self with another directory entry.
    fn cmp(&self, other: &DirEntry) -> Result<DirCmp, Error> {
        let mut results = Vec::with_capacity(self.entries.len());
        // compare each entry of the first directory with the content of
        // the second directory
        for (name, e1) in &self.entries {
            let diff = if let Some(e2) = other.entries.get(name) {
                e1.cmp(e2)
            } else {
                // the entry doesn't exist in the second directory
                Ok(EntryCmp::NotFound)
            };
            info!("Diff {:?} => {:?}", e1, diff);
            results.push(diff);
        }
        // check if all the entries are the same
        let is_same = results.into_iter().filter_map(|d| d.ok()).all(|d| match d {
            EntryCmp::Dir(dir) => dir == DirCmp::Same,
            EntryCmp::File(file) => file == FileCmp::Same,
            _ => false,
        });
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
            Err(err_msg("The given file does not exist!"))
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
enum Entry {
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

    /// Gets the path of the entry.
    fn path(&self) -> &Path {
        match self {
            Entry::Dir(e) => &e.path,
            Entry::File(e) => &e.path,
        }
    }

    /// Compares self with another entry.
    fn cmp(&self, other: &Entry) -> Result<EntryCmp, Error> {
        trace!("Compare: {} - {}", self, other);
        match (self, other) {
            (Entry::Dir(dir1), Entry::Dir(dir2)) => Ok(EntryCmp::Dir(dir1.cmp(dir2)?)),
            (Entry::File(f1), Entry::File(f2)) => Ok(EntryCmp::File(f1.cmp(f2)?)),
            _ => Err(err_msg("Cannot compare a directory with a file!")),
        }
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.path().display())
    }
}

/// Visits the given directory and build an Entry dta structure representing the
/// file system.
fn visit_dir(root: &Path) -> Result<Entry, Error> {
    let mut dir = Entry::new_dir(root)?;
    visit_dir_rec(&mut dir)?;
    Ok(dir)
}

fn visit_dir_rec(entry: &mut Entry) -> Result<(), Error> {
    // check that the entry is a directory
    match entry {
        Entry::Dir(directory) => {
            for e in fs::read_dir(&directory.path)? {
                let e = e?;
                let path = e.path();

                // get the path filename if any
                let file_name = path
                    .file_name()
                    .and_then(|p| p.to_str())
                    .map(|s| PathBuf::from(s))
                    .ok_or(err_msg("Cannot get the filename!"))?;

                if path.is_dir() {
                    trace!("New sub-directory: {:?}", path);
                    let mut dir = Entry::new_dir(&path)?;
                    // dfs with recursion
                    visit_dir_rec(&mut dir)?;
                    directory.entries.insert(file_name, dir);
                } else if path.is_file() {
                    trace!("New file: {:?}", path);
                    directory.entries.insert(file_name, Entry::new_file(&path)?);
                }
            }
            Ok(())
        }
        _ => Err(err_msg("Entry must be a directory!")),
    }
}
