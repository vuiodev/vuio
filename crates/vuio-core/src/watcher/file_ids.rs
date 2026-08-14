//! A bounded file-id cache for the debounced watcher.
//!
//! `notify-debouncer-full` pairs the two halves of a rename by comparing file
//! system ids, because the `from` path no longer exists by the time the event
//! arrives. Its own [`FileIdMap`](notify_debouncer_full::FileIdMap) does that by
//! walking every watched root and keeping a `HashMap<PathBuf, FileId>` entry for
//! every file and directory underneath — a recursive walk plus a `stat` per entry
//! at startup, and roughly half a kilobyte of resident memory per file for as long
//! as the server runs. Measured on a 500,000-file library that is a twelve-second
//! walk and about 250 MB held forever.
//!
//! Only backends without rename cookies need it at all: inotify carries a cookie
//! that pairs the halves directly, which is why upstream uses `NoCache` on Linux.
//! macOS and Windows have no cookie, so ids are the only mechanism there.
//!
//! What a paired rename buys is also smaller than it looks. A renamed *directory*
//! is handled by removing the old subtree and rescanning the new one, which is what
//! the unpaired delete-then-create pair produces anyway. A renamed *file* is the
//! only real difference: pairing keeps its row id stable and skips one tag re-read.
//!
//! So this keeps upstream's behaviour while it is cheap and stops paying for it
//! when it is not: the seed walk stops at [`DEFAULT_CAPACITY`] entries, and past
//! that point ids are remembered only for paths the watcher has actually seen an
//! event for. Libraries below the cap behave exactly as before. Above it, renaming
//! a file the watcher has not touched degrades to delete-then-create, which the
//! event handler already supports.

use notify::RecursiveMode;
use notify_debouncer_full::file_id::{get_file_id, FileId};
use notify_debouncer_full::FileIdCache;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::debug;

/// How many paths to remember. Two generations are kept, so the ceiling is twice
/// this — about 25 MB at the default, against 250 MB unbounded at 500k files.
pub const DEFAULT_CAPACITY: usize = 25_000;

/// A `FileIdCache` that never grows with the library.
///
/// Entries live in two generations. Inserts land in `live`; when it fills, it
/// becomes `previous` and a fresh one takes over, so the oldest half is dropped
/// wholesale rather than tracked with per-entry LRU bookkeeping. Lookups check
/// both. See the module docs for what this trades away.
#[derive(Debug)]
pub struct BoundedFileIdCache {
    live: HashMap<PathBuf, FileId>,
    previous: HashMap<PathBuf, FileId>,
    capacity: usize,
    /// Set once the seed walk has been cut short, so it is logged only the once.
    truncated: bool,
}

impl BoundedFileIdCache {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            live: HashMap::new(),
            previous: HashMap::new(),
            capacity: capacity.max(1),
            truncated: false,
        }
    }

    /// Entries across both generations.
    pub fn len(&self) -> usize {
        self.live.len() + self.previous.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert one path, rotating the generations if `live` is full.
    fn remember(&mut self, path: PathBuf, id: FileId) {
        if self.live.len() >= self.capacity && !self.live.contains_key(&path) {
            self.previous = std::mem::take(&mut self.live);
        }
        self.live.insert(path, id);
    }

    /// Seed ids for a tree that already exists, stopping at the cap.
    ///
    /// Unlike `remember` this never rotates: rotating mid-walk would let a large
    /// library evict its own entries and walk to the end for nothing.
    fn seed(&mut self, root: &Path, recursive: bool) {
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                if self.len() >= self.capacity {
                    if !self.truncated {
                        self.truncated = true;
                        debug!(
                            "File id cache reached its {} entry cap while seeding {}; \
                             renames outside the cache will be seen as delete + create",
                            self.capacity,
                            root.display()
                        );
                    }
                    return;
                }
                let path = entry.path();
                if let Ok(id) = get_file_id(&path) {
                    self.live.insert(path.clone(), id);
                }
                if recursive && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    pending.push(path);
                }
            }
        }
    }
}

impl Default for BoundedFileIdCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FileIdCache for BoundedFileIdCache {
    fn cached_file_id(&self, path: &Path) -> Option<impl AsRef<FileId>> {
        self.live.get(path).or_else(|| self.previous.get(path))
    }

    fn add_path(&mut self, path: &Path, recursive_mode: RecursiveMode) {
        if path.is_dir() {
            self.seed(path, recursive_mode == RecursiveMode::Recursive);
            return;
        }
        if let Ok(id) = get_file_id(path) {
            self.remember(path.to_path_buf(), id);
        }
    }

    fn remove_path(&mut self, path: &Path) {
        self.live.retain(|cached, _| !cached.starts_with(path));
        self.previous.retain(|cached, _| !cached.starts_with(path));
    }

    /// Deliberately does nothing.
    ///
    /// Upstream re-walks every root when the backend drops events — the moment the
    /// system is already under load. The dropped events are handled where it
    /// matters instead: the watcher marks those roots dirty and the media service
    /// rescans them against the index, which needs no file ids.
    fn rescan(&mut self, _roots: &[(PathBuf, RecursiveMode)]) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_files(dir: &Path, count: usize) {
        for index in 0..count {
            fs::write(dir.join(format!("file_{index}.mp3")), b"x").unwrap();
        }
    }

    #[test]
    fn seeding_stops_at_the_cap() {
        let temp = tempfile::tempdir().unwrap();
        write_files(temp.path(), 40);

        let mut cache = BoundedFileIdCache::with_capacity(10);
        cache.add_path(temp.path(), RecursiveMode::Recursive);

        assert_eq!(cache.len(), 10, "the seed walk must not exceed the cap");
    }

    #[test]
    fn seeding_a_small_tree_keeps_every_entry() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("Album");
        fs::create_dir(&nested).unwrap();
        write_files(temp.path(), 3);
        write_files(&nested, 3);

        let mut cache = BoundedFileIdCache::with_capacity(100);
        cache.add_path(temp.path(), RecursiveMode::Recursive);

        // 3 files + the directory + 3 nested files.
        assert_eq!(cache.len(), 7);
        assert!(cache
            .cached_file_id(&nested.join("file_0.mp3"))
            .is_some());
    }

    #[test]
    fn non_recursive_seeding_stays_at_one_level() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("Album");
        fs::create_dir(&nested).unwrap();
        write_files(&nested, 3);

        let mut cache = BoundedFileIdCache::with_capacity(100);
        cache.add_path(temp.path(), RecursiveMode::NonRecursive);

        assert_eq!(cache.len(), 1, "only the directory entry itself");
    }

    #[test]
    fn individual_inserts_stay_bounded_and_keep_the_newest() {
        let temp = tempfile::tempdir().unwrap();
        write_files(temp.path(), 30);

        let mut cache = BoundedFileIdCache::with_capacity(10);
        for index in 0..30 {
            cache.add_path(&temp.path().join(format!("file_{index}.mp3")), RecursiveMode::NonRecursive);
        }

        assert!(cache.len() <= 20, "two generations of 10, at most");
        assert!(
            cache
                .cached_file_id(&temp.path().join("file_29.mp3"))
                .is_some(),
            "the most recent path must still be resolvable"
        );
    }

    #[test]
    fn removing_a_directory_drops_its_children_from_both_generations() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("Album");
        fs::create_dir(&nested).unwrap();
        write_files(&nested, 6);

        let mut cache = BoundedFileIdCache::with_capacity(3);
        for index in 0..6 {
            cache.add_path(&nested.join(format!("file_{index}.mp3")), RecursiveMode::NonRecursive);
        }
        assert!(cache.len() > 0);

        cache.remove_path(&nested);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn rescan_does_not_rewalk() {
        let temp = tempfile::tempdir().unwrap();
        write_files(temp.path(), 5);

        let mut cache = BoundedFileIdCache::with_capacity(100);
        cache.rescan(&[(temp.path().to_path_buf(), RecursiveMode::Recursive)]);

        assert!(cache.is_empty(), "a dropped-event rescan must not walk");
    }
}
