use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;


use crate::hash::{hash_file, DuplicateGroup};

/// A job representing a single file to be hashed.
pub struct HashJob {
    pub path: PathBuf,
    pub size: u64,
}

/// The result returned by a worker after hashing a file.
pub struct HashResult {
    pub path: PathBuf,
    pub size: u64,
    pub hash_result: io::Result<blake3::Hash>,
}

/// A custom thread-pool worker for parallel file hashing.
pub struct WorkerPool {
    threads: usize,
}

impl WorkerPool {
    /// Creates a new worker pool with the specified thread count (at least 1).
    pub fn new(threads: usize) -> Self {
        Self {
            threads: threads.max(1),
        }
    }

    /// Hashes all files in candidate groups concurrently using `threads` worker threads.
    ///
    /// - Job queue: `Arc<Mutex<Receiver<HashJob>>>` allowing multiple worker consumers on standard `mpsc`.
    /// - Result queue: `mpsc` channel sending `HashResult` back to the collector.
    /// - Worker shutdown: Dropping the primary `job_sender` causes `recv()` to return `Err`,
    ///   allowing worker threads to exit cleanly.
    pub fn hash_candidates_parallel(
        &self,
        size_candidates: &HashMap<u64, Vec<PathBuf>>,
    ) -> Vec<DuplicateGroup> {
        let (job_tx, job_rx) = channel::<HashJob>();
        let (res_tx, res_rx) = channel::<HashResult>();

        let shared_rx = Arc::new(Mutex::new(job_rx));
        let mut handles = Vec::with_capacity(self.threads);

        for _ in 0..self.threads {
            let rx = Arc::clone(&shared_rx);
            let tx = res_tx.clone();

            let handle = thread::spawn(move || {
                loop {
                    // Acquire lock briefly only to pull next job from queue
                    let job = {
                        let lock = rx.lock().expect("worker lock poisoned");
                        match lock.recv() {
                            Ok(job) => job,
                            Err(_) => break, // Channel hung up / empty: gracefully shutdown
                        }
                    };

                    // Release lock BEFORE hashing so files are read in parallel!
                    let hash_result = hash_file(&job.path);
                    let _ = tx.send(HashResult {
                        path: job.path,
                        size: job.size,
                        hash_result,
                    });
                }
            });

            handles.push(handle);
        }

        // Drop the extra result sender held by the main thread so receiver knows when workers are done
        drop(res_tx);

        // Queue all candidate jobs
        for (&size, paths) in size_candidates {
            if paths.len() >= 2 {
                for path in paths {
                    let _ = job_tx.send(HashJob {
                        path: path.clone(),
                        size,
                    });
                }
            }
        }

        // Drop sender so workers see channel closure once queue is drained
        drop(job_tx);

        // Collect results from workers
        // Key: (size, hash) -> list of paths
        let mut hash_buckets: HashMap<(u64, blake3::Hash), Vec<PathBuf>> = HashMap::new();

        for result in res_rx {
            match result.hash_result {
                Ok(hash) => {
                    hash_buckets
                        .entry((result.size, hash))
                        .or_default()
                        .push(result.path);
                }
                Err(err) => {
                    eprintln!(
                        "[WARN] Failed to hash {}: {}",
                        result.path.display(),
                        err
                    );
                }
            }
        }

        // Wait for all worker threads to join
        for handle in handles {
            let _ = handle.join();
        }

        // Assemble duplicate groups (where 2 or more files have matching hash & size)
        let mut duplicate_groups = Vec::new();
        for ((file_size, hash), mut files) in hash_buckets {
            if files.len() > 1 {
                // Sort paths for deterministic output
                files.sort();
                duplicate_groups.push(DuplicateGroup {
                    hash,
                    file_size,
                    files,
                });
            }
        }

        // Sort groups deterministically by file_size descending, then hash
        duplicate_groups.sort_by(|a, b| {
            b.file_size
                .cmp(&a.file_size)
                .then_with(|| a.hash.as_bytes().cmp(b.hash.as_bytes()))
        });

        duplicate_groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walk;

    #[test]
    fn test_worker_pool_empty_candidates() {
        let pool = WorkerPool::new(4);
        let candidates = HashMap::new();
        let dupes = pool.hash_candidates_parallel(&candidates);
        assert!(dupes.is_empty());
    }

    #[test]
    fn test_parallel_matches_single_threaded() {
        let fixture_path = PathBuf::from("tests/fixtures");
        if !fixture_path.exists() {
            return;
        }

        let scanned = walk::walk(&fixture_path);
        let candidates = walk::filter_candidate_groups(walk::group_by_size(scanned));

        let single_pool = WorkerPool::new(1);
        let multi_pool = WorkerPool::new(4);

        let dupes_1 = single_pool.hash_candidates_parallel(&candidates);
        let dupes_4 = multi_pool.hash_candidates_parallel(&candidates);

        assert_eq!(dupes_1, dupes_4);
        assert!(!dupes_1.is_empty());
    }
}

