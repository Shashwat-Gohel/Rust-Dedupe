use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use dedupe::{pool, report, snapshot, walk};


struct TestEnv {
    path: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dedupe_test_{}_{}", name, unique));
        fs::create_dir_all(&path).expect("failed to create test temp dir");
        Self { path }
    }

    fn file(&self, rel: &str, content: &[u8]) -> PathBuf {
        let full = self.path.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(&full).unwrap();
        f.write_all(content).unwrap();
        full
    }

    fn dir(&self, rel: &str) -> PathBuf {
        let full = self.path.join(rel);
        fs::create_dir_all(&full).unwrap();
        full
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_integration_duplicates_and_empty_files() {
    let env = TestEnv::new("dupes");

    // Create 3 identical 128KB files (spans multiple 64KB chunks!)
    let large_payload = vec![0xABu8; 128 * 1024];
    env.file("large_1.bin", &large_payload);
    env.file("sub/large_2.bin", &large_payload);
    env.file("sub/nested/large_3.bin", &large_payload);

    // Create unique file of same size but different content
    let mut different_large = vec![0xABu8; 128 * 1024];
    different_large[500] = 0xCD; // single byte change
    env.file("diff_large.bin", &different_large);

    // Create 0-byte empty file
    env.file("empty.txt", b"");

    // Create empty subdirectory
    env.dir("empty_folder");

    // 1. Walk and size grouping
    let scanned = walk::walk(&env.path);
    // All 5 files should be scanned
    assert_eq!(scanned.len(), 5);

    let size_groups = walk::group_by_size(scanned);
    // Empty file (0 bytes) must be excluded from duplicate candidate groups
    assert!(!size_groups.contains_key(&0));

    // 128KB size group should have 4 files (3 dupes + 1 diff)
    let candidates = walk::filter_candidate_groups(size_groups);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates.get(&(128 * 1024)).unwrap().len(), 4);

    // 2. Parallel hashing
    let worker_pool = pool::WorkerPool::new(4);
    let dupes = worker_pool.hash_candidates_parallel(&candidates);

    // Only 1 duplicate group should remain (the 3 identical files)
    assert_eq!(dupes.len(), 1);
    let group = &dupes[0];
    assert_eq!(group.files.len(), 3);
    assert_eq!(group.file_size, 128 * 1024);
    assert_eq!(group.reclaimable_bytes(), 2 * 128 * 1024);

    // 3. Storage audit
    let audit = report::audit_storage(&env.path, std::time::Duration::from_secs(3600 * 24));
    assert_eq!(audit.empty_files.len(), 1);
    assert_eq!(audit.empty_dirs.len(), 1);
}

#[test]
fn test_integration_snapshot_and_diff_flow() {
    let env = TestEnv::new("snap_diff");

    env.file("stay.txt", b"staying unchanged");
    env.file("to_delete.txt", b"goodbye world");
    env.file("to_move.txt", b"content moving to another path");

    // Snapshot 1
    let snap1 = snapshot::Snapshot::create(&env.path).unwrap();
    let snap1_path = env.path.join("snap1.json");
    snap1.save_to_file(&snap1_path).unwrap();

    // Modify state:
    // 1. Delete to_delete.txt
    fs::remove_file(env.path.join("to_delete.txt")).unwrap();
    // 2. Move to_move.txt -> sub/moved.txt
    let move_content = fs::read(env.path.join("to_move.txt")).unwrap();
    fs::remove_file(env.path.join("to_move.txt")).unwrap();
    env.file("sub/moved.txt", &move_content);
    // 3. Add brand_new.txt
    env.file("brand_new.txt", b"brand new addition");
    // 4. Modify stay.txt
    env.file("stay.txt", b"modified content now");

    // Remove snap1.json from scan directory so it isn't indexed in snap2
    fs::remove_file(&snap1_path).unwrap();

    let snap2 = snapshot::Snapshot::create(&env.path).unwrap();

    let diff = snap1.diff(&snap2);

    assert_eq!(diff.added, vec![env.path.join("brand_new.txt")]);
    assert_eq!(diff.removed, vec![env.path.join("to_delete.txt")]);
    assert_eq!(diff.modified, vec![env.path.join("stay.txt")]);
    assert_eq!(
        diff.moved,
        vec![(env.path.join("to_move.txt"), env.path.join("sub/moved.txt"))]
    );
}
