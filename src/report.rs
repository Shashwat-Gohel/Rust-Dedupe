use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Formats raw byte counts into human-readable strings (B, KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

use serde::{Deserialize, Serialize};

/// Statistics for a specific file extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionStat {
    pub ext: String,
    pub count: usize,
    pub total_bytes: u64,
}

/// Returns the top N largest files sorted by size descending.
pub fn top_largest_files(files: &[(PathBuf, u64)], n: usize) -> Vec<(PathBuf, u64)> {
    let mut sorted = files.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().take(n).collect()
}

/// Directory storage aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirStat {
    pub path: PathBuf,
    pub total_bytes: u64,
    pub file_count: usize,
}

/// Aggregates storage by immediate containing directory, returning top directories sorted by size.
pub fn directory_size_aggregation(files: &[(PathBuf, u64)], top_n: usize) -> Vec<DirStat> {
    let mut dir_map: HashMap<PathBuf, (u64, usize)> = HashMap::new();

    for (file_path, size) in files {
        let parent = file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let entry = dir_map.entry(parent).or_insert((0, 0));
        entry.0 += size;
        entry.1 += 1;
    }

    let mut stats: Vec<DirStat> = dir_map
        .into_iter()
        .map(|(path, (total_bytes, file_count))| DirStat {
            path,
            total_bytes,
            file_count,
        })
        .collect();

    stats.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
    stats.into_iter().take(top_n).collect()
}

/// Computes file counts and cumulative size grouped by file extension.
pub fn extension_breakdown(files: &[(PathBuf, u64)]) -> Vec<ExtensionStat> {
    let mut ext_map: HashMap<String, (usize, u64)> = HashMap::new();

    for (file_path, size) in files {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "<none>".to_string());

        let entry = ext_map.entry(ext).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += size;
    }

    let mut result: Vec<ExtensionStat> = ext_map
        .into_iter()
        .map(|(ext, (count, total_bytes))| ExtensionStat {
            ext,
            count,
            total_bytes,
        })
        .collect();

    result.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
    result
}

/// Represents an audit of stale, empty, or special files.
pub struct StorageAudit {
    pub empty_files: Vec<PathBuf>,
    pub empty_dirs: Vec<PathBuf>,
    pub stale_files: Vec<(PathBuf, u64, SystemTime)>,
}

/// Scans root for empty files, empty directories, and stale files older than `stale_threshold`.
pub fn audit_storage(root: &Path, stale_threshold: Duration) -> StorageAudit {
    let mut empty_files = Vec::new();
    let mut empty_dirs = Vec::new();
    let mut stale_files = Vec::new();
    let now = SystemTime::now();

    audit_dir_recursive(root, &mut empty_files, &mut empty_dirs, &mut stale_files, now, stale_threshold);

    StorageAudit {
        empty_files,
        empty_dirs,
        stale_files,
    }
}

fn audit_dir_recursive(
    dir: &Path,
    empty_files: &mut Vec<PathBuf>,
    empty_dirs: &mut Vec<PathBuf>,
    stale_files: &mut Vec<(PathBuf, u64, SystemTime)>,
    now: SystemTime,
    stale_threshold: Duration,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut is_empty = true;

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };

        is_empty = false;
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            audit_dir_recursive(&path, empty_files, empty_dirs, stale_files, now, stale_threshold);
        } else if metadata.is_file() {
            let size = metadata.len();
            if size == 0 {
                empty_files.push(path.clone());
            }

            if let Ok(mtime) = metadata.modified() {
                if let Ok(age) = now.duration_since(mtime) {
                    if age >= stale_threshold {
                        stale_files.push((path, size, mtime));
                    }
                }
            }
        }
    }

    if is_empty {
        empty_dirs.push(dir.to_path_buf());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 5), "5.00 GB");
    }

    #[test]
    fn test_top_largest_files() {
        let files = vec![
            (PathBuf::from("small.txt"), 10),
            (PathBuf::from("huge.bin"), 5000),
            (PathBuf::from("medium.png"), 200),
        ];

        let top = top_largest_files(&files, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, PathBuf::from("huge.bin"));
        assert_eq!(top[1].0, PathBuf::from("medium.png"));
    }

    #[test]
    fn test_extension_breakdown() {
        let files = vec![
            (PathBuf::from("a.txt"), 100),
            (PathBuf::from("b.TXT"), 50),
            (PathBuf::from("img.png"), 300),
            (PathBuf::from("Makefile"), 40),
        ];

        let stats = extension_breakdown(&files);
        assert_eq!(stats[0].ext, "png");
        assert_eq!(stats[0].total_bytes, 300);

        let txt = stats.iter().find(|s| s.ext == "txt").unwrap();
        assert_eq!(txt.count, 2);
        assert_eq!(txt.total_bytes, 150);

        let none = stats.iter().find(|s| s.ext == "<none>").unwrap();
        assert_eq!(none.count, 1);
        assert_eq!(none.total_bytes, 40);
    }
}
