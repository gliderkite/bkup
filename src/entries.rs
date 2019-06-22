use failure::{err_msg, Error};
use fs_extra::dir;
use log::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

type EntryDeltaMap<'a> = HashMap<&'a Path, EntryDelta<'a>>;

/// Enumerates the possible results of a directory comparison.
#[derive(Debug, PartialEq)]
enum DirCmp {
    Same,
    Different,
}

/// Represents the delta between the directory entry it points to and the
/// directory entry it has been compared to.
#[derive(Debug, PartialEq)]
pub struct DirDelta<'a> {
    source: &'a DirEntry, // source directory entry used for the comparison
    dest: &'a DirEntry,   // destination directory entry used for the comparison
    diff: DirCmp, // comparison result between the directory source and the destination
    entries: EntryDeltaMap<'a>, // comparison results for each sub-entry
}

impl<'a> DirDelta<'a> {
    /// Creates a new directory difference from the given entries.
    fn new(
        source: &'a DirEntry,
        dest: &'a DirEntry,
        diff: DirCmp,
        entries: EntryDeltaMap<'a>,
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

    /// Gets an iterator over the directory entries.
    pub fn entries(&self) -> impl Iterator<Item = &EntryDelta<'a>> {
        self.entries.iter().map(|(_, e)| e)
    }
}

/// Represents the structure of a directory entry.
#[derive(Debug, PartialEq)]
pub struct DirEntry {
    // directory path
    path: PathBuf,
    // sub-entries where the key is the file name
    entries: HashMap<PathBuf, Entry>,
}

impl DirEntry {
    /// Creates a new directory entry by visiting it.
    fn new(path: &Path) -> Result<DirEntry, Error> {
        if path.is_dir() {
            let mut entry = DirEntry {
                path: path.to_path_buf(),
                entries: HashMap::new(),
            };
            entry.visit()?;
            Ok(entry)
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

    /// Visit and populate the directory entry.
    fn visit(&mut self) -> Result<(), Error> {
        // iterate over the directory entries
        for e in fs::read_dir(&self.path)? {
            let e = e?;
            let path = e.path();

            // get the entry filename if any
            let file_name = path.file_name().map(|s| PathBuf::from(s)).ok_or(
                format_err!("Cannot get the filename for '{:?}'", path),
            )?;

            if path.is_dir() {
                debug!("New sub-directory: {:?}", path);
                // dfs with recursion
                let dir = Entry::new_dir(&path)?;
                self.entries.insert(file_name, dir);
            } else if path.is_file() {
                debug!("New file: {:?}", path);
                self.entries
                    .insert(file_name, Entry::File(FileEntry::new(&path)?));
            }
        }
        Ok(())
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum EntryDelta<'a> {
    Dir(DirDelta<'a>),
    File(FileDelta<'a>),
    NotFound { entry: &'a Entry, path: PathBuf }, // `entry` not found in the path
}

impl<'a> EntryDelta<'a> {
    /// Updates the destination entry according to its given delta with the
    /// source entry.
    pub fn clear(&self) -> Result<(), Error> {
        match self {
            EntryDelta::Dir(delta) => {
                debug!("Directory delta: {:?}", delta);
                if !delta.is_none() {
                    for entry in delta.entries() {
                        entry.clear()?;
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
}

#[derive(Debug, PartialEq)]
pub enum Entry {
    // Directory
    Dir(DirEntry),
    // File
    File(FileEntry),
}

impl Entry {
    /// Creates a new entry that represents a directory and populates its
    /// entries by visiting it.
    pub fn new_dir(path: &Path) -> Result<Entry, Error> {
        Ok(Entry::Dir(DirEntry::new(path)?))
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

    /// Creates a new directory in the given root path.
    fn create_dir(root: &Path, name: &str) -> DirEntry {
        let dir: PathBuf = [root, Path::new(name)].iter().collect();
        fs::create_dir(&dir)
            .expect(&format!("Cannot create directory '{:?}'", dir));
        DirEntry::new(&dir)
            .expect(&format!("Cannot create DirEntry '{:?}'", dir))
    }

    /// Writes a new empty fule in the given root path.
    fn write_file(root: &Path, name: &str) -> FileEntry {
        let file: PathBuf = [root, Path::new(name)].iter().collect();
        thread::sleep(*SLEEP_INTERVAL);
        fs::write(&file, "")
            .expect(&format!("Cannot writes file '{:?}'", file));
        FileEntry::new(&file)
            .expect(&format!("Cannot create FileEntry '{:?}'", file))
    }

    /// Create the source and destination directories in a temp folder.
    fn create_source_and_dest_dirs() -> (DirEntry, DirEntry) {
        let temp_dir = env::temp_dir();;
        // create source and destination directories
        let source = Uuid::new_v4().to_simple().to_string();
        let source = create_dir(temp_dir.as_path(), &source);
        let dest = Uuid::new_v4().to_simple().to_string();
        let dest = create_dir(temp_dir.as_path(), &dest);
        (source, dest)
    }

    /// Asserts the given entry is marked as not found in the destination for
    /// the given directory delta.
    fn assert_entry_not_found_in_dest(
        delta: &DirDelta,
        entry_name: &str,
        count: usize,
    ) {
        assert!(delta.diff == DirCmp::Different);
        assert_eq!(delta.entries.len(), count);
        let entry_delta = delta
            .entries
            .get(Path::new(entry_name))
            .expect("Cannot get entry delta");
        match entry_delta {
            EntryDelta::NotFound { .. } => (),
            _ => panic!("Invalid delta"),
        }
    }

    /// Asserts that the given file is marked as found in the destination for
    /// the given directory delta, and its time difference with the source file
    /// is equal to the given one.
    fn assert_delta_cmp_with_file(
        delta: &DirDelta,
        delta_diff: DirCmp,
        file_name: &str,
        file_cmp: FileCmp,
        count: usize,
    ) {
        assert!(delta.diff == delta_diff);
        assert_eq!(delta.entries.len(), count);
        let entry_delta = delta
            .entries
            .get(Path::new(file_name))
            .expect("Cannot get entry delta");
        match entry_delta {
            EntryDelta::File(delta) => assert!(delta.diff == file_cmp),
            _ => panic!("Invalid delta"),
        }
    }

    /// Asserts that the given directory is marked as found in the destination for
    /// the given directory delta, and its time difference with the source
    /// directory is equal to the given one.
    fn assert_delta_cmp_with_dir(
        delta: &DirDelta,
        delta_diff: DirCmp,
        dir_name: &str,
        dir_cmp: DirCmp,
        count: usize,
    ) {
        assert!(delta.diff == delta_diff);
        assert_eq!(delta.entries.len(), count);
        let entry_delta = delta
            .entries
            .get(Path::new(dir_name))
            .expect("Cannot get entry delta");
        match entry_delta {
            EntryDelta::Dir(delta) => assert!(delta.diff == dir_cmp),
            _ => panic!("Invalid delta"),
        }
    }

    #[test]
    fn test_cmp_dir() {
        let (mut source, mut dest) = create_source_and_dest_dirs();
        let source_path = source.path().to_path_buf();
        let dest_path = dest.path().to_path_buf();

        // comparing an entry with itself should not show any difference
        let delta = source
            .cmp(&source)
            .expect("Cannot compare directory entries");
        assert!(delta.diff == DirCmp::Same);
        assert!(delta.entries.is_empty());
        // both with no files, the two directories are the same
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        assert!(delta.diff == DirCmp::Same);
        assert!(delta.entries.is_empty());

        // add one file to source
        let file1_name = "file1";
        write_file(&source_path, file1_name);

        // file1 exists only on the source
        source.visit().expect("Cannot visit source directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        assert_entry_not_found_in_dest(&delta, file1_name, 1);

        // but the two folders are the same when seen from the destination
        // (no entry in destination is missing in source)
        let delta =
            dest.cmp(&source).expect("Cannot compare directory entries");
        assert!(delta.diff == DirCmp::Same);
        assert!(delta.entries.is_empty());

        // add same file to destination
        write_file(&dest_path, file1_name);

        // file1 now exists in both directories
        dest.visit().expect("Cannot visit dest directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        // file i1 n source is older
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file1_name,
            FileCmp::Older,
            1,
        );
        let delta =
            dest.cmp(&source).expect("Cannot compare directory entries");
        // file 1 is newer in dest
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file1_name,
            FileCmp::Newer,
            1,
        );

        // add a new file in the destination directory
        let file2_name = "file2";
        write_file(&dest_path, file2_name);
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        // only file 1 is seen from source an it is older than file 1 in dest
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file1_name,
            FileCmp::Older,
            1,
        );
        dest.visit().expect("Cannot visit dest directory");
        let delta =
            dest.cmp(&source).expect("Cannot compare directory entries");
        // dest has 2 files and file 1 is newer that file 1 in source
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file1_name,
            FileCmp::Newer,
            2,
        );
        // file 2 only exist in dest
        assert_entry_not_found_in_dest(&delta, file2_name, 2);
    }

    #[test]
    fn test_cmp_sub_dir() {
        let (mut source, mut dest) = create_source_and_dest_dirs();

        // create subdirectory in source
        let dir1_name = "dir1";
        let source_dir1 = create_dir(source.path(), dir1_name);

        // dir 1 only exists in source
        source.visit().expect("Cannot visit source directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        assert_entry_not_found_in_dest(&delta, dir1_name, 1);

        // but the two folders are the same when seen from the destination
        // (no entry in destination is missing in source)
        let delta =
            dest.cmp(&source).expect("Cannot compare directory entries");
        assert!(delta.diff == DirCmp::Same);
        assert!(delta.entries.is_empty());

        // create dir1 in dest
        let dest_dir1 = create_dir(dest.path(), dir1_name);

        // dir 1 exists both in source and destination
        source.visit().expect("Cannot visit source directory");
        dest.visit().expect("Cannot visit dest directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        assert_delta_cmp_with_dir(
            &delta,
            DirCmp::Same,
            dir1_name,
            DirCmp::Same,
            1,
        );

        // create sub-dir in source
        let sub_dir1_name = "sub_dir1";
        let mut source_sub_dir1 = create_dir(source_dir1.path(), sub_dir1_name);
        source.visit().expect("Cannot visit source directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        // source and dest are different because dir 1 is different since it
        // contains a sub-directory only in source
        assert_delta_cmp_with_dir(
            &delta,
            DirCmp::Different,
            dir1_name,
            DirCmp::Different,
            1,
        );

        // but the two folders are the same when seen from the destination
        // (no entry in destination is missing in source)
        let delta =
            dest.cmp(&source).expect("Cannot compare directory entries");
        assert!(delta.diff == DirCmp::Same);
        assert_eq!(delta.entries.len(), 1);

        // create sub-dir in dest
        let mut dest_sub_dir1 = create_dir(dest_dir1.path(), sub_dir1_name);
        dest.visit().expect("Cannot visit dest directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        // both source and dest contain the same entries
        assert!(delta.diff == DirCmp::Same);
        assert_eq!(delta.entries.len(), 1);

        // add file 1 to source sub-directory
        let file1_name = "file1";
        write_file(source_sub_dir1.path(), file1_name);
        source.visit().expect("Cannot visit source directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        // source and dest are different because dir 1 is different since it
        // contains a sub-directory that has files only in source
        assert_delta_cmp_with_dir(
            &delta,
            DirCmp::Different,
            dir1_name,
            DirCmp::Different,
            1,
        );

        // add file 1 and file 2 to dest sub directory and then file 2 to source,
        // so that file 1 is newer in source but file 2 is newer in dest
        let file2_name = "file2";
        write_file(dest_sub_dir1.path(), file1_name);
        write_file(dest_sub_dir1.path(), file2_name);
        write_file(source_sub_dir1.path(), file2_name);
        source.visit().expect("Cannot visit source directory");
        dest.visit().expect("Cannot visit dest directory");
        let delta =
            source.cmp(&dest).expect("Cannot compare directory entries");
        // source and dest are different because the files contained in both
        // directories are the same but their timestamps are different
        assert_delta_cmp_with_dir(
            &delta,
            DirCmp::Different,
            dir1_name,
            DirCmp::Different,
            1,
        );

        // compare the sub-directories with files
        source_sub_dir1
            .visit()
            .expect("Cannot visit source directory");
        dest_sub_dir1.visit().expect("Cannot visit dest directory");

        // source vs dest
        let delta = source_sub_dir1
            .cmp(&dest_sub_dir1)
            .expect("Cannot compare directory entries");
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file1_name,
            FileCmp::Older,
            2,
        );
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file2_name,
            FileCmp::Newer,
            2,
        );

        // dest vs source
        let delta = dest_sub_dir1
            .cmp(&source_sub_dir1)
            .expect("Cannot compare directory entries");
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file1_name,
            FileCmp::Newer,
            2,
        );
        assert_delta_cmp_with_file(
            &delta,
            DirCmp::Different,
            file2_name,
            FileCmp::Older,
            2,
        );
    }

    #[test]
    fn test_cmp_files() {
        let temp_dir = env::temp_dir();
        // create older file
        let older = Uuid::new_v4().to_simple().to_string();
        let older = write_file(&temp_dir, &older);
        // create newer file
        let newer = Uuid::new_v4().to_simple().to_string();
        let newer = write_file(&temp_dir, &newer);

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
