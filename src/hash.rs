use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

pub const CHUNK_SIZE: usize = 64 * 1024; // 64 KB chunk size

/// Represents a group of identical files sharing the same content hash and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub hash: blake3::Hash,
    pub file_size: u64,
    pub files: Vec<PathBuf>,
}

impl DuplicateGroup {
    /// Reclaimable bytes if keeping 1 original and removing all duplicates.
    pub fn reclaimable_bytes(&self) -> u64 {
        if self.files.len() > 1 {
            (self.files.len() as u64 - 1) * self.file_size
        } else {
            0
        }
    }
}

/// Hashes a file in fixed 64 KB chunks using `BufReader` and `blake3::Hasher`.
///
/// Streaming chunks ensures constant O(1) memory usage regardless of file size.
pub fn hash_file(path: &Path) -> io::Result<blake3::Hash> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(CHUNK_SIZE, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; CHUNK_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

/// Identifies true duplicate files across same-size candidate groups.
///
/// Only files belonging to size groups with 2 or more files are hashed.
/// Any read error on an individual file is logged to `stderr` without aborting.
pub fn find_duplicates(
    size_candidates: &HashMap<u64, Vec<PathBuf>>,
) -> Vec<DuplicateGroup> {
    let mut duplicate_groups = Vec::new();

    for (&file_size, paths) in size_candidates {
        if paths.len() < 2 {
            continue;
        }

        let mut hash_buckets: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();

        for path in paths {
            match hash_file(path) {
                Ok(hash) => {
                    hash_buckets.entry(hash).or_default().push(path.clone());
                }
                Err(err) => {
                    eprintln!(
                        "[WARN] Failed to read/hash file {}: {}",
                        path.display(),
                        err
                    );
                }
            }
        }

        for (hash, files) in hash_buckets {
            if files.len() > 1 {
                duplicate_groups.push(DuplicateGroup {
                    hash,
                    file_size,
                    files,
                });
            }
        }
    }

    duplicate_groups
}

/// Computes the total reclaimable space (in bytes) across all duplicate groups.
pub fn total_reclaimable_space(groups: &[DuplicateGroup]) -> u64 {
    groups.iter().map(|g| g.reclaimable_bytes()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reclaimable_bytes() {
        let group = DuplicateGroup {
            hash: blake3::hash(b"test"),
            file_size: 1024,
            files: vec![
                PathBuf::from("a"),
                PathBuf::from("b"),
                PathBuf::from("c"),
            ],
        };
        // 3 files of 1024 bytes -> keeping 1 means (3 - 1) * 1024 = 2048 bytes reclaimable
        assert_eq!(group.reclaimable_bytes(), 2048);
    }

    #[test]
    fn test_single_file_no_reclaimable() {
        let group = DuplicateGroup {
            hash: blake3::hash(b"test"),
            file_size: 1024,
            files: vec![PathBuf::from("a")],
        };
        assert_eq!(group.reclaimable_bytes(), 0);
    }
}
