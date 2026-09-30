# dedupe: TODO

Rust disk audit CLI: duplicates, largest files, type breakdown, snapshot/diff.

Rule for every module: write it (AI-assisted), explain it without looking, get quizzed on it, then rewrite one function from scratch. Don't tick a box until you can explain it.

---

## Phase 0: Setup (15 min)
- [x] `cargo new dedupe && cd dedupe`
- [x] `git init`, first commit
- [x] Add deps: `blake3`, `serde`, `serde_json`, `clap` (derive)
- [x] Create test folder with known duplicates, nested dirs, empty files, a symlink

## Phase 1: Directory walk + size grouping (1.5 hr)
- [x] Recursive `walk()` collecting `(PathBuf, u64)`
- [x] Skip symlinks (`symlink_metadata`)
- [x] Report unreadable dirs/files to stderr, keep going
- [x] Group by size in `HashMap<u64, Vec<PathBuf>>`, skip 0-byte files
- [x] Print groups with more than one file
- [x] Understand: why `symlink_metadata`? how does `.entry().or_default()` work? why `&mut Vec`?

## Phase 2: Hashing (1.5 hr)
- [x] Hash a file in fixed chunks (64 KB) with `BufReader` + `blake3::Hasher`
- [x] Hash only files in same-size groups
- [x] Group by hash, keep groups with more than one file
- [x] Compute reclaimable space per group and in total
- [x] Handle read errors per file without crashing
- [x] Understand: why chunks vs `read_to_end`? what does memory look like for a 20 GB file?

## Phase 3: Parallel hashing (2 hr)
- [x] Worker pool with `thread::spawn`
- [x] Job distribution: `mpsc` channel + `Arc<Mutex<Receiver>>`
- [x] Results collected back over a second channel
- [x] `--threads N` flag
- [x] Verify output is identical to single-threaded
- [x] Understand: what do `Send`/`Sync` do here? why `Arc<Mutex<...>>`? how do workers shut down?

## Phase 4: Reports (1.5 hr)
- [x] `--top N` largest files
- [x] Per-directory size aggregation (largest folders)
- [x] Breakdown by extension: count + total size
- [x] Human-readable sizes (KB/MB/GB)
- [x] Optional: stale files (mtime older than N months), empty files/dirs

## Phase 5: Snapshot and diff (3 hr)
- [x] Define `FileEntry { path, size, hash, mtime }` with `serde` derive
- [x] `snapshot <path> -o snap.json`
- [x] `diff old.json new.json` reports added / removed / modified
- [x] Moved-file detection: same hash, different path
- [x] Edge case: duplicates sharing a hash when detecting moves
- [x] Understand: why hash-based move detection? what breaks with duplicate files?

## Phase 6: CLI structure (1 hr)
- [x] `clap` subcommands: `scan`, `report`, `snapshot`, `diff`
- [x] Shared flags: `--min-size`, `--json`
- [x] Clear error messages and non-zero exit codes
- [x] Split code into modules: `walk.rs`, `hash.rs`, `pool.rs`, `report.rs`, `snapshot.rs`

## Phase 7: Testing (1 hr)
- [x] Unit tests: size grouping, hash grouping, diff logic
- [x] Integration test using a temp directory with known duplicates
- [x] Test edge cases: empty dir, unreadable file, symlink, same content in different sizes (should not exist), large file


## Phase 8: Benchmark (1 hr)
- [ ] Build a realistic test set (thousands of files, some large)
- [ ] Measure: no size filter vs size filter (hash count and time)
- [ ] Measure: 1 vs 2 vs 4 vs 8 threads
- [ ] Record results in a table (use your own numbers only)
- [ ] Note why speedup flattens (disk I/O)

## Phase 9: README (45 min)
- [ ] One-paragraph description
- [ ] Usage examples with real output
- [ ] Architecture diagram (walk -> size group -> hash pool -> report)
- [ ] Benchmark table
- [ ] Design decisions and trade-offs
- [ ] Limitations (Unix-only if you use inode features, symlinks skipped, etc.)
- [ ] Mention AI assistance honestly

## Phase 10: Interview prep (1 hr)
Write answers in your own words:
- [ ] Why group by size first? What did it save in your benchmark?
- [ ] Why chunked hashing? What about a 20 GB file?
- [ ] Hash collisions: how likely, and when would you byte-compare?
- [ ] Why is thread speedup limited? (I/O-bound vs CPU-bound)
- [ ] How does Rust prevent data races in the worker pool?
- [ ] How does move detection work and where does it fail?
- [ ] How would you scale it? (partial hashing, hash cache by mtime, hardlink detection)
- [ ] What would you do differently with more time?
- [ ] Do the rewrite test once per module

---

## Stretch (only if time remains)
- [ ] Partial hash (first 4 KB) stage before full hash
- [ ] Hardlink detection via inode + device
- [ ] Byte-by-byte verification of final groups
- [ ] Progress bar with `indicatif`
- [ ] `--dry-run` delete mode

## Resume checklist
- [ ] Every number in bullets comes from your own benchmark
- [ ] Every listed feature is built and explainable
- [ ] Repo pushed to GitHub with README