use failure::{err_msg, Error};
use fs_extra::dir;
use log::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

type EntryCmpMap<'a> = HashMap<&'a Path, EntryDelta<'a>>;

/// Enumerates the possible results of a directory comparison.
#[derive(Debug, PartialEq)]
enum DirCmp {
    Same,
    Different,
}

/// Represents the delta between the directory entry it points to and the
/// directory entry it has been compared to.
#[derive(Debug)]
pub struct DirDelta<'a> {
    source: &'a DirEntry, // source directory entry used for the comparison
    dest: &'a DirEntry,   // destination directory entry used for the comparison
    diff: DirCmp, // comparison result between the directory source and the destination
    entries: EntryCmpMap<'a>, // comparison results for each sub-entry
}

impl<'a> DirDelta<'a> {
    /// Creates a new directory difference from the given entries.
    fn new(
        source: &'a DirEntry,
        dest: &'a DirEntry,
        diff: DirCmp,
        entries: EntryCmpMap<'a>,
    ) -> Self {
        DirDelta {
            source,
            dest,
            diff,
            entries,
        }
    }

    /// Returns true only if there is no delta between the source and destination.
    pub fn is_none(&self) -> bool {
        self.diff == DirCmp::Same
    }

    pub fn entries(&self) -> impl Iterator<Item = &EntryDelta<'a>> {
        self.entries.iter().map(|(_, e)| e)
    }
}

/// Represents the structure of a directory entry.
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

    /// Compares self with another directory entry and returns the delta.
    fn cmp<'a>(&'a self, other: &'a DirEntry) -> Result<DirDelta<'a>, Error> {
        let mut entries = HashMap::with_capacity(self.entries.len());
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
                Ok(EntryDelta::NotFound {
                    entry: e1,
                    path: dest_path,
                })
            };
            debug!("Difference for {:?}: {:?}", e1, cmp_res);
            let cmp_res = cmp_res?;

            // check if all the entries are the same by finding the first difference
            if is_same {
                is_same = match &cmp_res {
                    EntryDelta::Dir(dir) => dir.diff == DirCmp::Same,
                    EntryDelta::File(file) => file.diff == FileCmp::Same,
                    _ => false,
                };
            }

            entries.insert(name.as_path(), cmp_res);
        }

        let diff = if is_same {
            DirCmp::Same
        } else {
            DirCmp::Different
        };
        Ok(DirDelta::new(self, other, diff, entries))
    }

    /// Gets the directory path.
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

/// Enumerates the possible results of a file comparison.
#[derive(Debug, PartialEq)]
enum FileCmp {
    Same,
    Older,
    Newer,
}

/// Represents the delta between the file entry it points to and the file entry
/// it has been compared to.
#[derive(Debug)]
pub struct FileDelta<'a> {
    source: &'a FileEntry, // source file entry used for the comparison
    dest: &'a FileEntry,   // destination file entry used for the comparison
    diff: FileCmp,         // comparison result
}

impl<'a> FileDelta<'a> {
    /// Creates a new file delta from the given entries.
    fn new(source: &'a FileEntry, dest: &'a FileEntry, diff: FileCmp) -> Self {
        FileDelta { source, dest, diff }
    }

    /// Returns true only if the source is newer than destination.
    pub fn is_newer(&self) -> bool {
        self.diff == FileCmp::Newer
    }

    /// Gets the source file entry.
    pub fn source(&self) -> &'a FileEntry {
        self.source
    }

    /// Gets the destination file entry.
    pub fn destination(&self) -> &'a FileEntry {
        self.dest
    }
}

/// Represents a file entry.
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

    /// Copies self into the given destination.
    pub fn copy(&self, dest: &Path) -> Result<FileEntry, Error> {
        info!("Copy file {:?} to {:?}", self.path, dest);
        fs::copy(&self.path, dest)?;
        FileEntry::new(dest)
    }

    /// Compares self with another file entry.
    fn cmp<'a>(&'a self, other: &'a FileEntry) -> Result<FileDelta<'a>, Error> {
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
                let diff = match t1.cmp(&t2) {
                    Ordering::Less => FileCmp::Older,
                    Ordering::Greater => FileCmp::Newer,
                    Ordering::Equal => FileCmp::Same,
                };
                Ok(FileDelta::new(self, other, diff))
            }
            _ => Err(format_err!(
                "Invalid filenames for {:?} {:?}!",
                path1,
                path2
            )),
        }
    }

    /// Gets the file path.
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

#[derive(Debug)]
pub enum EntryDelta<'a> {
    Dir(DirDelta<'a>),
    File(FileDelta<'a>),
    NotFound { entry: &'a Entry, path: PathBuf }, // `entry` not found in the path
}

#[derive(Debug)]
pub enum Entry {
    // Directory
    Dir(DirEntry),
    // File
    File(FileEntry),
}

impl Entry {
    /// Creates a new entry that represents a directory and populates its
    /// entries by visiting it.
    pub fn visit_dir(path: &Path) -> Result<Entry, Error> {
        let mut entry = Entry::Dir(DirEntry::new(path)?);
        entry.visit()?;
        Ok(entry)
    }

    /// Gets the path of the entry.
    fn path(&self) -> &Path {
        match self {
            Entry::Dir(e) => e.path(),
            Entry::File(e) => e.path(),
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
    pub fn copy(&self, dest: &Path) -> Result<Entry, Error> {
        match self {
            Entry::Dir(e) => Ok(Entry::Dir(e.copy(dest)?)),
            Entry::File(e) => Ok(Entry::File(e.copy(dest)?)),
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
                        // dfs with recursion
                        let dir = Entry::visit_dir(&path)?;
                        directory.entries.insert(file_name, dir);
                    } else if path.is_file() {
                        debug!("New file: {:?}", path);
                        directory.entries.insert(
                            file_name,
                            Entry::File(FileEntry::new(&path)?),
                        );
                    }
                }
                Ok(())
            }
            _ => Err(err_msg("Cannot visit a file!")),
        }
    }

    /// Compares self with another entry.
    pub fn cmp<'a>(
        &'a self,
        other: &'a Entry,
    ) -> Result<EntryDelta<'a>, Error> {
        debug!("Comparing: {} to {}", self, other);
        match (self, other) {
            (Entry::Dir(dir1), Entry::Dir(dir2)) => {
                let delta = dir1.cmp(dir2)?;
                Ok(EntryDelta::Dir(delta))
            }
            (Entry::File(f1), Entry::File(f2)) => {
                let delta = f1.cmp(f2)?;
                Ok(EntryDelta::File(delta))
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
        let delta = older.cmp(&newer).expect("Cannot compare entries");
        assert_eq!(delta.diff, FileCmp::Older);
        let delta = older.cmp(&older).expect("Cannot compare entries");
        assert_eq!(delta.diff, FileCmp::Same);
        let delta = newer.cmp(&older).expect("Cannot compare entries");
        assert_eq!(delta.diff, FileCmp::Newer);
        let delta = newer.cmp(&newer).expect("Cannot compare entries");
        assert_eq!(delta.diff, FileCmp::Same);

        // create a copy of the older file
        let copy = older
            .copy(newer.path.as_path())
            .expect("Cannot create a copy");
        let delta = older.cmp(&copy).expect("Cannot compare entries");
        assert_eq!(delta.diff, FileCmp::Older);
        let delta = copy.cmp(&older).expect("Cannot compare entries");
        assert_eq!(delta.diff, FileCmp::Newer);
    }
}
