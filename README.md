# dedupe

A high-performance, memory-efficient disk auditing and file deduplication CLI built in Rust. It locates duplicate files across nested directory structures, calculates reclaimable storage space, generates filesystem usage reports, and tracks file additions, modifications, and renames across point-in-time cryptographic snapshots.

---

## Architecture

The deduplication engine is organized into a four-stage pipeline designed to minimize disk I/O and memory overhead:

```
+-----------------------------------------------------------------------------------+
| 1. Directory Walk (walk.rs)                                                       |
|    - Recursive traversal skipping symlinks via `symlink_metadata`                |
|    - Collects `(PathBuf, u64)` while logging unreadable paths to stderr           |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 2. Size Grouping (walk.rs)                                                        |
|    - Buckets files into `HashMap<u64, Vec<PathBuf>>`                              |
|    - Filters out 0-byte files and single-file size buckets                        |
|    - Eliminates ~93%+ of unique files before any disk reads or hashing             |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 3. Parallel Chunked Hashing (pool.rs & hash.rs)                                   |
|    - Multi-worker thread pool (`thread::spawn`, `mpsc`, `Arc<Mutex<Receiver>>`)   |
|    - Streaming 64 KB chunk hashing via `BufReader` + `blake3::Hasher`             |
|    - Constant O(1) RAM consumption independent of file size                       |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 4. Deduplication & Reports (main.rs, report.rs, snapshot.rs)                      |
|    - Groups identical content hashes & computes reclaimable space                 |
|    - Formatted terminal tables or structured JSON (`--json`)                      |
|    - Differential snapshot audits (`snapshot` & `diff`)                          |
+-----------------------------------------------------------------------------------+
```

---

## Benchmark Results

Benchmarks were executed in `--release` mode on a realistic synthetic dataset composed of **1,602 files (13.92 MB total volume)** with 1,500 unique files, 15 duplicate groups (6 copies each), and 3 large binary groups (up to 1.5 MB each).

### Experiment 1: Size Filtering vs. Unfiltered Hashing
| Strategy | Files Hashed | Total Execution Time | Files Avoided | % Work Saved |
| :--- | :---: | :---: | :---: | :---: |
| **No Filter (Hash All)** | 1,602 | 68.48 ms | 0 | 0.0% |
| **Size Pre-Filter** | 103 | 7.65 ms | 1,499 files | **93.6%** |

> **Result:** Grouping by size before hashing delivered an **~8.95x speedup**, bypassing disk reads for 93.6% of the files in the directory.

### Experiment 2: Multi-Threading Worker Pool Scaling
| Threads | Median Time (ms) | Speedup vs 1 Thread | Duplicate Groups Identified |
| :---: | :---: | :---: | :---: |
| **1** | 13.21 ms | 1.00x | 18 |
| **2** | 10.23 ms | 1.29x | 18 |
| **4** | 6.69 ms | 1.98x | 18 |
| **8** | 4.89 ms | **2.70x** | 18 |
| **12** | 5.22 ms | 2.53x | 18 |

#### Why Speedup Flattens
Scaling from 1 to 4 threads yields near-linear speedup (~2x). However, between 8 and 12 threads, performance plateaus and slightly regresses:
- **I/O Bound Saturation:** File reads are constrained by OS file table locks, filesystem driver context switches, and SSD queue depth rather than raw CPU cycles.
- **Worker Lock Contention:** The `Arc<Mutex<Receiver<HashJob>>>` job queue incurs atomic lock contention when tasks finish in sub-millisecond intervals.

---

## Installation & Usage

### Building from Source
Ensure Rust 1.80+ is installed:
```bash
cargo build --release
```
The compiled executable will be located at `target/release/dedupe.exe` (or `target/release/dedupe` on Unix).

---

### Commands

#### 1. Scan for Duplicate Files
Scans a target directory, groups by size, and hashes candidates in parallel:
```bash
dedupe scan ./my_folder --threads 4
```
Output:
```text
Scanning: ./my_folder
Worker threads: 4
Minimum file size: 1 bytes
Potential duplicate candidates: 2 groups (5 files to hash)
Hashing candidate files...

Found 1 duplicate group(s):

[Group 1] Size: 25 B | Hash: 8f85a5c3d632fe621c36c42f23a4d85f154a60ba7c7975b38de19dad75aeb19c
  Reclaimable: 50 B (2 redundant copies)
  - ./my_folder/file_a.txt
  - ./my_folder/file_b.txt
  - ./my_folder/nested/file_c.txt

Total Reclaimable Space: 50 B
```

To emit machine-readable JSON:
```bash
dedupe scan ./my_folder --json
```

#### 2. Storage & Hygiene Report
Inspects top space consumers, extension breakdown, and flags stale or empty files:
```bash
dedupe report ./my_folder --top 10 --stale-months 6
```
Output:
```text
=== DISK USAGE AUDIT REPORT ===
Directory: ./my_folder
Total regular files: 7

--- Top 7 Largest Files ---
        29 B  ./my_folder/unique.txt
        25 B  ./my_folder/file_a.txt
        25 B  ./my_folder/file_b.txt
        25 B  ./my_folder/nested/file_c.txt

--- Top Folders by Storage ---
        95 B (6 files)  ./my_folder
        25 B (1 files)  ./my_folder/nested

--- Breakdown by Extension ---
  .txt           7 files       120 B

--- Hygiene Audit ---
  Empty files: 1
  Empty directories: 1
  Stale files (> 6 months): 0
```

#### 3. Snapshot & Differential Auditing
Take a point-in-time cryptographic index:
```bash
dedupe snapshot ./my_project -o baseline.json
```
After making file changes, compare the new state against the baseline:
```bash
dedupe snapshot ./my_project -o updated.json
dedupe diff baseline.json updated.json
```
Output:
```text
=== SNAPSHOT DIFF ===
Old: baseline.json
New: updated.json
Unchanged files: 42

Added (1):
  + ./my_project/src/new_feature.rs

Removed (1):
  - ./my_project/docs/deprecated.txt

Modified (2):
  * ./my_project/Cargo.toml
  * ./my_project/src/main.rs

Moved / Renamed (1):
  > ./my_project/src/old_name.rs -> ./my_project/src/pool.rs
```

---

## Design Decisions & Trade-Offs

1. **Why BLAKE3 instead of MD5 / SHA-256?**
   - BLAKE3 is cryptographically secure (unlike MD5/SHA-1) and significantly faster than SHA-256 (up to 10x-15x faster on modern SIMD architectures).
   - Its tree-hashing structure provides exceptional throughput when hashing medium-to-large files.

2. **Chunked Streaming (64 KB) vs. `fs::read`:**
   - Loading files via `fs::read` or `read_to_end` requires allocating memory equal to the file size. For a 20 GB archive, this would consume 20 GB of RAM per worker thread, causing out-of-memory panics.
   - Using fixed 64 KB buffers with `BufReader` guarantees that memory usage is bounded to $O(\text{workers} \times 64\text{ KB})$, which is $\approx 1\text{ MB}$ even with 16 threads.

3. **Custom Thread Pool vs. `rayon`:**
   - While `rayon` provides simple parallel iterators, implementing our own worker pool using `std::thread::spawn`, `mpsc`, and `Arc<Mutex<Receiver<HashJob>>>` provides explicit control over job dispatching, thread life cycle, and channel closure.

4. **Hash-Based Move Detection vs. Path Heuristics:**
   - Moves and renames are identified when an added file and a removed file share the identical BLAKE3 hash and byte length.
   - *Duplicate move edge case:* If multiple files share the same content hash in `old` and some are moved or deleted, the diff engine tracks candidate sets per hash and performs 1-to-1 pairings to prevent false move reports.

---

## Limitations

- **Symlinks & Reparse Points:** Skipped by default via `symlink_metadata` to guarantee cycle prevention across NTFS directory junctions and Unix symlinks.
- **Inode / Hardlink Detection:** Does not currently deduplicate hardlinked files that point to identical filesystem inodes (hardlinks on Unix or NTFS file IDs).
- **In-Memory Size Map:** The initial directory walk stores file paths in a `Vec<(PathBuf, u64)>`. For filesystems with $\ge 50$ million files, streaming walk iterators (or SQLite/RocksDB disk backing) would be required.

---

## Attribution

Developed with pair-programming assistance from Antigravity AI following clean Rust idiomatic patterns.
