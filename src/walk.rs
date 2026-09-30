use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Recursively traverses `root` and collects all regular files along with their byte sizes.
///
/// - Uses `symlink_metadata` to avoid following symlinks or junctions (preventing cycles).
/// - Skips symlinks entirely.
/// - Catches and logs any permission/read errors to `stderr` and keeps walking.
pub fn walk(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    walk_dir_recursive(root, &mut files);
    files
}

fn walk_dir_recursive(dir: &Path, files: &mut Vec<(PathBuf, u64)>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("[WARN] Cannot read directory {}: {}", dir.display(), err);
            return;
        }
    };

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(err) => {
                eprintln!("[WARN] Failed reading entry in {}: {}", dir.display(), err);
                continue;
            }
        };

        let path = entry.path();

        // Query metadata WITHOUT following symlinks
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(err) => {
                eprintln!("[WARN] Cannot read metadata for {}: {}", path.display(), err);
                continue;
            }
        };

        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            // Skip symlinks to avoid infinite loops or aliased counting
            continue;
        }

        if file_type.is_dir() {
            walk_dir_recursive(&path, files);
        } else if file_type.is_file() {
            files.push((path, metadata.len()));
        }
    }
}

/// Groups files by exact file size in bytes, skipping 0-byte files.
///
/// Uses `HashMap::entry` API to group paths into a `Vec<PathBuf>` for each unique size.
pub fn group_by_size(files: Vec<(PathBuf, u64)>) -> HashMap<u64, Vec<PathBuf>> {
    let mut groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for (path, size) in files {
        if size == 0 {
            // Skip 0-byte files as specified
            continue;
        }
        groups.entry(size).or_default().push(path);
    }

    groups
}

/// Filters the size map to only keep candidate groups containing 2 or more files.
pub fn filter_candidate_groups(groups: HashMap<u64, Vec<PathBuf>>) -> HashMap<u64, Vec<PathBuf>> {
    groups
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_by_size_skips_zero_bytes() {
        let input = vec![
            (PathBuf::from("a.txt"), 100),
            (PathBuf::from("b.txt"), 0),
            (PathBuf::from("c.txt"), 100),
        ];

        let groups = group_by_size(input);
        assert!(!groups.contains_key(&0));
        assert_eq!(groups.get(&100).unwrap().len(), 2);
    }

    #[test]
    fn test_filter_candidate_groups() {
        let mut groups = HashMap::new();
        groups.insert(100, vec![PathBuf::from("a"), PathBuf::from("b")]);
        groups.insert(200, vec![PathBuf::from("c")]);

        let filtered = filter_candidate_groups(groups);
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key(&100));
        assert!(!filtered.contains_key(&200));
    }
}

