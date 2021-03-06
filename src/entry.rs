use failure::{err_msg, Error};
use ignore::gitignore::Gitignore;
use log::*;
use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

type EntryDeltaMap<'a> = HashMap<&'a Path, EntryDelta<'a>>;

/// Represents the delta between the directory entry it points to and the
/// directory entry it has been compared to.
#[derive(Debug, PartialEq)]
pub struct DirDelta<'a> {
    source: &'a DirEntry, // source directory entry used for the comparison
    dest: &'a DirEntry,   // destination directory entry used for the comparison
    entries: EntryDeltaMap<'a>, // comparison results for each sub-entry
}

impl<'a> DirDelta<'a> {
    /// Creates a new directory difference from the given entries.
    fn new(
        source: &'a DirEntry,
        dest: &'a DirEntry,
        entries: EntryDeltaMap<'a>,
    ) -> Self {
        DirDelta {
            source,
            dest,
            entries,
        }
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
    // sub-entries where the key is the entry name
    entries: HashMap<PathBuf, Entry>,
}

impl DirEntry {
    /// Creates a new directory entry by visiting it.
    /// If the `ignore` flags is set and a ".gitignore" file exists in the
    /// directory, it will be parsed to ignore all the specified files and folders.
    fn new<P: Into<PathBuf>>(path: P, ignore: bool) -> Result<DirEntry, Error> {
        let path = path.into();
        if path.is_dir() {
            let mut entry = DirEntry {
                path,
                entries: HashMap::new(),
            };
            let ignore = if ignore {
                let gitignore: PathBuf =
                    [&entry.path, Path::new(".gitignore")].iter().collect();
                let (ignore, _) = Gitignore::new(gitignore);
                Some(ignore)
            } else {
                None
            };
            entry.visit(ignore.as_ref())?;
            Ok(entry)
        } else {
            Err(format_err!("The given directory {:?} does not exist", path))
        }
    }

    /// Copies self into the given destination.
    fn copy(&self, dest: &Path) -> Result<(), Error> {
        info!("Copying directory {:?} to {:?}", self.path, dest);
        // create destination directory
        if !dest.is_dir() {
            fs::create_dir(dest)?;
        }
        // iterate over each source entry to copy it
        for (filename, entry) in &self.entries {
            let dest_entry: PathBuf =
                [dest, Path::new(filename)].iter().collect();
            match entry {
                Entry::Dir(dir) => {
                    dir.copy(&dest_entry)?;
                }
                Entry::File(file) => {
                    file.copy(&dest_entry)?;
                }
            }
        }
        Ok(())
    }

    /// Compares self with another directory entry and returns the delta.
    fn cmp<'a>(
        &'a self,
        other: &'a DirEntry,
        accuracy: &'a Duration,
    ) -> Result<Option<DirDelta<'a>>, Error> {
        let mut entries = HashMap::new();
        // compare each entry of the first directory with the content of
        // the second directory
        for (name, e1) in &self.entries {
            let delta = if let Some(e2) = other.entries.get(name) {
                e1.cmp(e2, accuracy)?
            } else {
                let dest_path: PathBuf =
                    [other.path.as_path(), e1.file_name()?].iter().collect();
                // the entry doesn't exist in the other directory
                Some(EntryDelta::NotFound {
                    entry: e1,
                    path: dest_path,
                })
            };
            debug!("Difference for {:?}: {:?}", e1, delta);
            // check if there is a difference between the compared entries
            if let Some(delta) = delta {
                entries.insert(name.as_path(), delta);
            }
        }

        // if no entries have been stored it means the given directories have no
        // differences
        let delta = if entries.is_empty() {
            None
        } else {
            Some(DirDelta::new(self, other, entries))
        };

        Ok(delta)
    }

    /// Visit and populate the directory entry.
    fn visit(&mut self, ignore: Option<&Gitignore>) -> Result<(), Error> {
        // iterate over the directory entries
        let dirs = fs::read_dir(&self.path)?.filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                warn!("Cannot read directory: {}", e);
                None
            }
        });

        for e in dirs {
            let path = e.path();
            let is_dir = path.is_dir();

            // check if this path must be ignored
            if let Some(ignore) = ignore {
                if ignore.matched(&path, is_dir).is_ignore() {
                    info!("Ignoring {:?}", path);
                    continue;
                }
            }

            // get the entry filename if any
            let file_name =
                path.file_name().map(PathBuf::from).ok_or_else(|| {
                    format_err!("Cannot get the filename for {:?}", path)
                })?;

            if is_dir {
                debug!("New sub-directory: {:?}", path);
                // dfs with recursion, carry ignore settings into sub-directory
                let dir = Entry::directory(&path, ignore.is_some())?;
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
enum FileTimeDelta {
    Older,
    Newer,
}

/// Represents the delta between the file entry it points to and the file entry
/// it has been compared to.
#[derive(Debug, PartialEq)]
pub struct FileDelta<'a> {
    source: &'a FileEntry, // source file entry used for the comparison
    dest: &'a FileEntry,   // destination file entry used for the comparison
    diff: FileTimeDelta,   // comparison result
}

impl<'a> FileDelta<'a> {
    /// Creates a new file delta from the given entries.
    fn new(
        source: &'a FileEntry,
        dest: &'a FileEntry,
        diff: FileTimeDelta,
    ) -> Self {
        FileDelta { source, dest, diff }
    }

    /// Returns true only if the source is newer than destination.
    pub fn is_newer(&self) -> bool {
        self.diff == FileTimeDelta::Newer
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
    fn new<P: Into<PathBuf>>(path: P) -> Result<FileEntry, Error> {
        let path = path.into();
        if path.is_file() {
            Ok(FileEntry { path })
        } else {
            Err(format_err!("The given file {:?} does not exist", path))
        }
    }

    /// Copies self into the given destination.
    pub fn copy(&self, dest: &Path) -> Result<(), Error> {
        info!("Copying file {:?} to {:?}", self.path, dest);
        fs::copy(self.path(), dest)?;
        Ok(())
    }

    /// Compares self with another file entry.
    fn cmp<'a>(
        &'a self,
        other: &'a FileEntry,
        accuracy: &'a Duration,
    ) -> Result<Option<FileDelta<'a>>, Error> {
        use std::time::UNIX_EPOCH;
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
                let t1 = fs::metadata(path1)?
                    .modified()?
                    .duration_since(UNIX_EPOCH)?;
                let t2 = fs::metadata(path2)?
                    .modified()?
                    .duration_since(UNIX_EPOCH)?;
                // compare timestamps
                let time_delta = FileEntry::cmp_modified(t1, t2, accuracy);
                let delta =
                    time_delta.map(|delta| FileDelta::new(self, other, delta));
                Ok(delta)
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

    /// Compares the source and destination modified times taking into account the
    /// given accuracy.
    fn cmp_modified(
        source: Duration,
        dest: Duration,
        accuracy: &Duration,
    ) -> Option<FileTimeDelta> {
        match source.cmp(&dest) {
            Ordering::Greater => {
                // source may be newer
                if (source - *accuracy) > dest {
                    Some(FileTimeDelta::Newer)
                } else {
                    None
                }
            }
            Ordering::Less => {
                // source may be older (dest may be newer)
                if (dest - *accuracy) > source {
                    Some(FileTimeDelta::Older)
                } else {
                    None
                }
            }
            Ordering::Equal => None,
        }
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
                for entry in delta.entries() {
                    entry.clear()?;
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
    pub fn directory<P: Into<PathBuf>>(
        path: P,
        ignore: bool,
    ) -> Result<Entry, Error> {
        Ok(Entry::Dir(DirEntry::new(path, ignore)?))
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
            .ok_or_else(|| {
                format_err!("Cannot get the filename for '{}'", self)
            })
    }

    /// Copies self into the given destination.
    fn copy(&self, dest: &Path) -> Result<(), Error> {
        match self {
            Entry::Dir(e) => e.copy(dest)?,
            Entry::File(e) => e.copy(dest)?,
        };
        Ok(())
    }

    /// Compares self with another entry.
    pub fn cmp<'a>(
        &'a self,
        other: &'a Entry,
        accuracy: &'a Duration,
    ) -> Result<Option<EntryDelta<'a>>, Error> {
        debug!(
            "Comparing: '{}' to '{}' ({:?} accuracy)",
            self, other, accuracy
        );
        match (self, other) {
            (Entry::Dir(dir1), Entry::Dir(dir2)) => {
                let delta = dir1.cmp(dir2, accuracy)?.map(EntryDelta::Dir);
                Ok(delta)
            }
            (Entry::File(f1), Entry::File(f2)) => {
                let delta = f1.cmp(f2, accuracy)?.map(EntryDelta::File);
                Ok(delta)
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
    use std::{env, thread, time};
    use uuid::Uuid;

    lazy_static! {
        /// Interval used to write files with significant difference on the
        /// modification time stored in the metadata.
        static ref ACCURACY: time::Duration = time::Duration::from_millis(2000);
    }

    // Empty gitignore matcher that never matches anything.
    const IGNORE: Option<&Gitignore> = None;

    #[test]
    fn test_cmp_dir() {
        let (mut source, mut dest) = create_source_and_dest_dirs();
        let source_path = source.path().to_path_buf();
        let dest_path = dest.path().to_path_buf();

        // comparing an entry with itself should not show any difference
        let delta = source
            .cmp(&source, &ACCURACY)
            .expect("Cannot compare directory entries");
        assert!(delta.is_none());
        // both with no files, the two directories are the same
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries");
        assert!(delta.is_none());

        // add one file to source
        let file1_name = "file1";
        write_file(&source_path, file1_name);

        // file1 exists only on the source
        source.visit(IGNORE).expect("Cannot visit source directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        assert_entry_not_found_in_dest(&delta, file1_name, 1);

        // but the two folders are the same when seen from the destination
        // (no entry in destination is missing in source)
        let delta = dest
            .cmp(&source, &ACCURACY)
            .expect("Cannot compare directory entries");
        assert!(delta.is_none());

        // add same file to destination
        write_file(&dest_path, file1_name);

        // file 1 now exists in both directories
        dest.visit(IGNORE).expect("Cannot visit dest directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // file 1 in source is older
        assert_delta_cmp_with_file(&delta, file1_name, FileTimeDelta::Older, 1);
        let delta = dest
            .cmp(&source, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // file 1 is newer in dest
        assert_delta_cmp_with_file(&delta, file1_name, FileTimeDelta::Newer, 1);

        // add a new file in the destination directory
        let file2_name = "file2";
        write_file(&dest_path, file2_name);
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // only file 1 is seen from source an it is older than file 1 in dest
        assert_delta_cmp_with_file(&delta, file1_name, FileTimeDelta::Older, 1);
        dest.visit(IGNORE).expect("Cannot visit dest directory");
        let delta = dest
            .cmp(&source, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // dest has 2 files and file 1 is newer that file 1 in source
        assert_delta_cmp_with_file(&delta, file1_name, FileTimeDelta::Newer, 2);
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
        source.visit(IGNORE).expect("Cannot visit source directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        assert_entry_not_found_in_dest(&delta, dir1_name, 1);

        // but the two folders are the same when seen from the destination
        // (no entry in destination is missing in source)
        let delta = dest
            .cmp(&source, &ACCURACY)
            .expect("Cannot compare directory entries");
        assert!(delta.is_none());

        // create dir1 in dest
        let dest_dir1 = create_dir(dest.path(), dir1_name);

        // dir 1 exists both in source and destination
        source.visit(IGNORE).expect("Cannot visit source directory");
        dest.visit(IGNORE).expect("Cannot visit dest directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries");
        assert!(delta.is_none());

        // create sub-dir in source
        let sub_dir1_name = "sub_dir1";
        let mut source_sub_dir1 = create_dir(source_dir1.path(), sub_dir1_name);
        source.visit(IGNORE).expect("Cannot visit source directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // source and dest are different because dir 1 is different since it
        // contains a sub-directory only in source
        assert_delta_cmp_with_dir(&delta, dir1_name, 1);

        // but the two folders are the same when seen from the destination
        // (no entry in destination is missing in source)
        let delta = dest
            .cmp(&source, &ACCURACY)
            .expect("Cannot compare directory entries");
        assert!(delta.is_none());

        // create sub-dir in dest
        let mut dest_sub_dir1 = create_dir(dest_dir1.path(), sub_dir1_name);
        dest.visit(IGNORE).expect("Cannot visit dest directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries");
        // both source and dest contain the same entries
        assert!(delta.is_none());

        // add file 1 to source sub-directory
        let file1_name = "file1";
        write_file(source_sub_dir1.path(), file1_name);
        source.visit(IGNORE).expect("Cannot visit source directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // source and dest are different because dir 1 is different since it
        // contains a sub-directory that has files only in source
        assert_delta_cmp_with_dir(&delta, dir1_name, 1);

        // add file 1 and file 2 to dest sub directory and then file 2 to source,
        // so that file 1 is newer in source but file 2 is newer in dest
        let file2_name = "file2";
        write_file(dest_sub_dir1.path(), file1_name);
        write_file(dest_sub_dir1.path(), file2_name);
        write_file(source_sub_dir1.path(), file2_name);
        source.visit(IGNORE).expect("Cannot visit source directory");
        dest.visit(IGNORE).expect("Cannot visit dest directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        // source and dest are different because the files contained in both
        // directories are the same but their timestamps are different
        assert_delta_cmp_with_dir(&delta, dir1_name, 1);

        // compare the sub-directories with files
        source_sub_dir1
            .visit(IGNORE)
            .expect("Cannot visit source directory");
        dest_sub_dir1
            .visit(IGNORE)
            .expect("Cannot visit dest directory");

        // source vs dest
        let delta = source_sub_dir1
            .cmp(&dest_sub_dir1, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        assert_delta_cmp_with_file(&delta, file1_name, FileTimeDelta::Older, 2);
        assert_delta_cmp_with_file(&delta, file2_name, FileTimeDelta::Newer, 2);

        // dest vs source
        let delta = dest_sub_dir1
            .cmp(&source_sub_dir1, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        assert_delta_cmp_with_file(&delta, file1_name, FileTimeDelta::Newer, 2);
        assert_delta_cmp_with_file(&delta, file2_name, FileTimeDelta::Older, 2);
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
        let delta = older
            .cmp(&newer, &ACCURACY)
            .expect("Cannot compare entries")
            .expect("Delta should be some");
        assert_eq!(delta.diff, FileTimeDelta::Older);
        let delta = older
            .cmp(&older, &ACCURACY)
            .expect("Cannot compare entries");
        assert!(delta.is_none());
        let delta = newer
            .cmp(&older, &ACCURACY)
            .expect("Cannot compare entries")
            .expect("Delta should be some");
        assert_eq!(delta.diff, FileTimeDelta::Newer);
        let delta = newer
            .cmp(&newer, &ACCURACY)
            .expect("Cannot compare entries");
        assert!(delta.is_none());

        // create a copy of the older file
        older
            .copy(newer.path.as_path())
            .expect("Cannot create a copy");
        let copy = FileEntry::new(newer.path.as_path())
            .expect("Cannot create FileEntry");
        let delta =
            older.cmp(&copy, &ACCURACY).expect("Cannot compare entries");
        assert!(delta.is_none() || delta.unwrap().diff == FileTimeDelta::Older);
        let delta =
            copy.cmp(&older, &ACCURACY).expect("Cannot compare entries");
        assert!(delta.is_none() || delta.unwrap().diff == FileTimeDelta::Newer);
    }

    #[test]
    fn test_entries_to_ignore() {
        let (mut source, dest) = create_source_and_dest_dirs();
        let source_path = source.path().to_path_buf();

        let ignore_filename = ".gitignore";
        let filename_to_ignore = "ignore.txt";

        // create .gitignore file in source directory
        let ignore_path: PathBuf =
            [source_path.as_path(), Path::new(ignore_filename)]
                .iter()
                .collect();
        fs::write(&ignore_path, filename_to_ignore).expect("Cannot write file");
        let (ignore, _) = Gitignore::new(ignore_path);

        // add another file to source
        write_file(&source_path, filename_to_ignore);

        // file1 exists only on the source but since it has to be ignored the
        // only difference must be the .gitignore file itself
        source
            .visit(Some(&ignore))
            .expect("Cannot visit source directory");
        let delta = source
            .cmp(&dest, &ACCURACY)
            .expect("Cannot compare directory entries")
            .expect("Delta should be some");
        assert_entry_not_found_in_dest(&delta, ignore_filename, 1);
    }

    /// Creates a new directory in the given root path.
    fn create_dir(root: &Path, name: &str) -> DirEntry {
        let dir: PathBuf = [root, Path::new(name)].iter().collect();
        fs::create_dir(&dir)
            .unwrap_or_else(|_| panic!("Cannot create directory {:?}", dir));
        let ignore = false;
        DirEntry::new(&dir, ignore)
            .unwrap_or_else(|_| panic!("Cannot create DirEntry {:?}", dir))
    }

    /// Writes a new empty fule in the given root path.
    fn write_file(root: &Path, name: &str) -> FileEntry {
        let file: PathBuf = [root, Path::new(name)].iter().collect();
        thread::sleep(*ACCURACY + Duration::from_millis(10));
        fs::write(&file, "")
            .unwrap_or_else(|_| panic!("Cannot writes file {:?}", file));
        FileEntry::new(&file)
            .unwrap_or_else(|_| panic!("Cannot create FileEntry {:?}", file))
    }

    /// Create the source and destination directories in a temp folder.
    fn create_source_and_dest_dirs() -> (DirEntry, DirEntry) {
        let temp_dir = env::temp_dir();
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
        file_name: &str,
        file_cmp: FileTimeDelta,
        count: usize,
    ) {
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
        dir_name: &str,
        count: usize,
    ) {
        assert_eq!(delta.entries.len(), count);
        let entry_delta = delta
            .entries
            .get(Path::new(dir_name))
            .expect("Cannot get entry delta");
        match entry_delta {
            EntryDelta::Dir(_) => (),
            _ => panic!("Invalid delta"),
        }
    }
}
