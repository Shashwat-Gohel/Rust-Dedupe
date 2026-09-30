use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::hash::hash_file;
use crate::walk;

/// Represents a single indexed file entry in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub hash: String,
    pub mtime: u64,
}

/// Represents a point-in-time filesystem snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub root: PathBuf,
    pub created_at: u64,
    pub files: Vec<FileEntry>,
}

/// Differential audit report comparing two snapshots.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffReport {
    pub added: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub moved: Vec<(PathBuf, PathBuf)>, // (old_path, new_path)
    pub unchanged_count: usize,
}

impl Snapshot {
    /// Creates a snapshot of `root` by walking files and computing cryptographic hashes.
    pub fn create(root: &Path) -> io::Result<Self> {
        let scanned = walk::walk(root);
        let mut files = Vec::new();

        for (path, size) in scanned {
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(err) => {
                    eprintln!("[WARN] Cannot read metadata for {}: {}", path.display(), err);
                    continue;
                }
            };

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let hash = match hash_file(&path) {
                Ok(h) => h.to_hex().to_string(),
                Err(err) => {
                    eprintln!("[WARN] Failed to hash {}: {}", path.display(), err);
                    continue;
                }
            };

            files.push(FileEntry {
                path,
                size,
                hash,
                mtime,
            });
        }

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Snapshot {
            root: root.to_path_buf(),
            created_at,
            files,
        })
    }

    /// Saves snapshot to JSON file.
    pub fn save_to_file(&self, output_path: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(output_path, json)
    }

    /// Loads a snapshot from a JSON file.
    pub fn load_from_file(input_path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(input_path)?;
        let snapshot = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(snapshot)
    }

    /// Computes the diff between this snapshot (considered `old`) and another snapshot (`new`).
    pub fn diff(&self, new_snap: &Snapshot) -> DiffReport {
        let mut report = DiffReport::default();

        let old_by_path: HashMap<&PathBuf, &FileEntry> =
            self.files.iter().map(|f| (&f.path, f)).collect();
        let new_by_path: HashMap<&PathBuf, &FileEntry> =
            new_snap.files.iter().map(|f| (&f.path, f)).collect();

        let mut candidate_removed: Vec<&FileEntry> = Vec::new();
        let mut candidate_added: Vec<&FileEntry> = Vec::new();

        // 1. Check entries in old
        for (path, old_entry) in &old_by_path {
            if let Some(new_entry) = new_by_path.get(path) {
                if old_entry.hash == new_entry.hash && old_entry.size == new_entry.size {
                    report.unchanged_count += 1;
                } else {
                    report.modified.push((*path).clone());
                }
            } else {
                candidate_removed.push(old_entry);
            }
        }

        // 2. Check entries in new that do not exist at same path in old
        for (path, new_entry) in &new_by_path {
            if !old_by_path.contains_key(path) {
                candidate_added.push(new_entry);
            }
        }

        // 3. Move detection: match candidate_removed with candidate_added by (hash, size)
        // Group candidate removed and added by hash
        let mut removed_by_hash: HashMap<&str, Vec<&FileEntry>> = HashMap::new();
        for entry in candidate_removed {
            removed_by_hash.entry(&entry.hash).or_default().push(entry);
        }

        let mut matched_added_paths: HashSet<&PathBuf> = HashSet::new();

        for new_entry in &candidate_added {
            if let Some(removed_list) = removed_by_hash.get_mut(new_entry.hash.as_str()) {
                if let Some(old_entry) = removed_list.pop() {
                    report
                        .moved
                        .push((old_entry.path.clone(), new_entry.path.clone()));
                    matched_added_paths.insert(&new_entry.path);
                }
            }
        }

        // Remaining unmatched removed entries are true deletions
        for (_hash, remaining_removed) in removed_by_hash {
            for entry in remaining_removed {
                report.removed.push(entry.path.clone());
            }
        }

        // Remaining unmatched added entries are true additions
        for entry in candidate_added {
            if !matched_added_paths.contains(&entry.path) {
                report.added.push(entry.path.clone());
            }
        }

        // Sort outputs for consistency
        report.added.sort();
        report.removed.sort();
        report.modified.sort();
        report.moved.sort();

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_added_removed_modified_moved() {
        let old_snap = Snapshot {
            root: PathBuf::from("."),
            created_at: 1000,
            files: vec![
                FileEntry {
                    path: PathBuf::from("keep.txt"),
                    size: 10,
                    hash: "h_keep".into(),
                    mtime: 100,
                },
                FileEntry {
                    path: PathBuf::from("edit.txt"),
                    size: 20,
                    hash: "h_edit_1".into(),
                    mtime: 100,
                },
                FileEntry {
                    path: PathBuf::from("del.txt"),
                    size: 30,
                    hash: "h_del".into(),
                    mtime: 100,
                },
                FileEntry {
                    path: PathBuf::from("old_pos.txt"),
                    size: 40,
                    hash: "h_move".into(),
                    mtime: 100,
                },
            ],
        };

        let new_snap = Snapshot {
            root: PathBuf::from("."),
            created_at: 2000,
            files: vec![
                FileEntry {
                    path: PathBuf::from("keep.txt"),
                    size: 10,
                    hash: "h_keep".into(),
                    mtime: 100,
                },
                FileEntry {
                    path: PathBuf::from("edit.txt"),
                    size: 25,
                    hash: "h_edit_2".into(),
                    mtime: 200,
                },
                FileEntry {
                    path: PathBuf::from("new_file.txt"),
                    size: 50,
                    hash: "h_new".into(),
                    mtime: 200,
                },
                FileEntry {
                    path: PathBuf::from("sub/new_pos.txt"),
                    size: 40,
                    hash: "h_move".into(),
                    mtime: 100,
                },
            ],
        };

        let diff = old_snap.diff(&new_snap);

        assert_eq!(diff.unchanged_count, 1);
        assert_eq!(diff.modified, vec![PathBuf::from("edit.txt")]);
        assert_eq!(diff.removed, vec![PathBuf::from("del.txt")]);
        assert_eq!(diff.added, vec![PathBuf::from("new_file.txt")]);
        assert_eq!(
            diff.moved,
            vec![(PathBuf::from("old_pos.txt"), PathBuf::from("sub/new_pos.txt"))]
        );
    }

    #[test]
    fn test_duplicate_move_edge_case() {
        // Two files in old had identical content. One was moved/renamed.
        let old_snap = Snapshot {
            root: PathBuf::from("."),
            created_at: 1000,
            files: vec![
                FileEntry {
                    path: PathBuf::from("orig1.txt"),
                    size: 10,
                    hash: "shared_hash".into(),
                    mtime: 100,
                },
                FileEntry {
                    path: PathBuf::from("orig2.txt"),
                    size: 10,
                    hash: "shared_hash".into(),
                    mtime: 100,
                },
            ],
        };

        let new_snap = Snapshot {
            root: PathBuf::from("."),
            created_at: 2000,
            files: vec![
                // orig1 still exists
                FileEntry {
                    path: PathBuf::from("orig1.txt"),
                    size: 10,
                    hash: "shared_hash".into(),
                    mtime: 100,
                },
                // orig2 was moved to orig2_renamed.txt
                FileEntry {
                    path: PathBuf::from("orig2_renamed.txt"),
                    size: 10,
                    hash: "shared_hash".into(),
                    mtime: 100,
                },
            ],
        };

        let diff = old_snap.diff(&new_snap);
        assert_eq!(diff.unchanged_count, 1);
        assert_eq!(
            diff.moved,
            vec![(PathBuf::from("orig2.txt"), PathBuf::from("orig2_renamed.txt"))]
        );
        assert!(diff.removed.is_empty());
        assert!(diff.added.is_empty());
    }
}
