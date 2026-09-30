<div align="center">

# **dedupe**


**High-Performance Disk Audit & Deduplication Engine**

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![Blake3](https://img.shields.io/badge/Hash-BLAKE3-blue)](https://github.com/BLAKE3-team/BLAKE3)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)]()
[![Version](https://img.shields.io/badge/Version-0.1.0-blueviolet)]()

</div>

---

## Overview

**Dedupe** is a blazing-fast, memory-efficient file deduplication and disk audit CLI written in Rust. It identifies duplicate files across arbitrarily deep directory trees using parallel BLAKE3 content hashing, generates actionable disk usage reports, and provides cryptographic point-in-time filesystem snapshots with differential auditing.

> Built for developers, sysadmins, and power users who need accurate, high-speed disk analysis without relying on slow GUI tools or language runtimes.

---

## Key Features

| Feature | Description |
|---|---|
| 🔍 **Duplicate Detection** | Size-based pre-filter + parallel BLAKE3 hashing for 8–9x speedup |
| 📊 **Disk Usage Report** | Top files, top directories, extension breakdown, hygiene audit |
| 📸 **Snapshots** | Cryptographic point-in-time directory index (JSON) |
| 🔀 **Diff Engine** | Detects added, removed, modified, and moved/renamed files |
| 🖥️ **Interactive TUI** | Full terminal GUI with folder picker, scrollable results, Recycle Bin integration |
| ⚡ **Rust Performance** | Zero-copy streaming, O(1) memory, configurable worker thread pool |
| 🗃️ **JSON Output** | Machine-readable `--json` flag for all commands |

---

## Architecture

The deduplication engine runs a four-stage pipeline designed to minimize disk I/O and memory allocation:

```
+---------------------------------------------------------------------+
|  Stage 1: Directory Walk  (walk.rs)                                 |
|  Recursive traversal, skips symlinks via symlink_metadata           |
|  Collects (PathBuf, u64) pairs, logs unreadable paths to stderr     |
+---------------------------------------------------------------------+
                             |
                             v
+---------------------------------------------------------------------+
|  Stage 2: Size Grouping  (walk.rs)                                  |
|  Buckets files into HashMap<u64, Vec<PathBuf>>                      |
|  Eliminates ~93%+ of unique files before any disk reads             |
+---------------------------------------------------------------------+
                             |
                             v
+---------------------------------------------------------------------+
|  Stage 3: Parallel Chunked Hashing  (pool.rs + hash.rs)            |
|  Multi-worker thread pool: thread::spawn + mpsc + Arc<Mutex<Rx>>   |
|  Streaming 64 KB chunks via BufReader + blake3::Hasher              |
|  Constant O(1) RAM regardless of file size                          |
+---------------------------------------------------------------------+
                             |
                             v
+---------------------------------------------------------------------+
|  Stage 4: Dedup + Reports + Snapshots  (main.rs, report.rs,        |
|           snapshot.rs)                                              |
|  Groups identical hashes, computes reclaimable space               |
|  Formatted terminal output or structured JSON (--json)             |
|  Differential snapshot auditing (snapshot + diff)                  |
+---------------------------------------------------------------------+
```

### Source Layout

```
src/
├── main.rs       — CLI entrypoint, command dispatch, JSON serialization
├── walk.rs       — Recursive directory walk + size-bucket grouping
├── hash.rs       — BLAKE3 streaming file hasher (64 KB chunks)
├── pool.rs       — Custom multi-worker thread pool (mpsc + Arc<Mutex>)
├── report.rs     — Disk usage report: top files, dirs, extensions, hygiene
└── snapshot.rs   — Point-in-time JSON snapshots + differential diff engine
```

---

## Benchmark Results

All benchmarks run in `--release` mode on a synthetic dataset:
**1,602 files · 13.92 MB total · 15 duplicate groups (6 copies each)**

### Size Pre-Filter vs. Unfiltered Hashing

| Strategy | Files Hashed | Execution Time | Work Saved |
|:---|:---:|:---:|:---:|
| Hash All (no filter) | 1,602 | 68.48 ms | 0.0% |
| **Size Pre-Filter** | **103** | **7.65 ms** | **93.6%** |

> The size-grouping stage delivers an **~8.95x speedup** by discarding 93.6% of files before any hashing occurs.

### Worker Thread Pool Scaling

| Threads | Median Time | Speedup | Groups Found |
|:---:|:---:|:---:|:---:|
| 1 | 13.21 ms | 1.00x | 18 |
| 2 | 10.23 ms | 1.29x | 18 |
| 4 | 6.69 ms | 1.98x | 18 |
| **8** | **4.89 ms** | **2.70x** | 18 |
| 12 | 5.22 ms | 2.53x | 18 |

> Performance plateaus above 8 threads due to SSD I/O queue depth saturation and lock contention on the `Arc<Mutex<Receiver>>` job channel.

---

## Installation

### Prerequisites

- [Rust 1.80+](https://rustup.rs/) — `rustup install stable`

### Build from Source

```bash
git clone https://github.com/ShashwatGohel/dedupe.git
cd dedupe
cargo build --release
```

The binary will be at `target/release/dedupe` (or `dedupe.exe` on Windows).

### Optional: Add to PATH

```bash
# Linux / macOS
cp target/release/dedupe /usr/local/bin/

# Windows (PowerShell)
Copy-Item target\release\dedupe.exe "$env:USERPROFILE\bin\"
```

---

## Usage

### Interactive TUI (Windows)

The easiest way to use dedupe is via the built-in **Terminal UI**. Just double-click or run:

```bat
dedupe-gui.bat
```

Features of the TUI:
- Native **Windows folder picker** dialogs (Desktop, Downloads, all drives & network accessible)
- **Scrollable results** with Up/Down arrows, Page Up/Down, Home/End
- **One-key Recycle Bin** integration — press `D` to safely send duplicates to Recycle Bin (fully recoverable)
- **FIGlet ASCII banner** rendered dynamically in-terminal
- **Zero dependencies** — single `.bat` file, no install or runtime needed

---

### CLI Reference

#### `scan` — Find Duplicate Files

```bash
dedupe scan <DIRECTORY> [OPTIONS]

Options:
  --threads <N>      Worker thread count (default: CPU count)
  --min-size <N>     Minimum file size in bytes (default: 1)
  --json             Emit machine-readable JSON
```

**Example:**

```bash
dedupe scan ./projects --threads 8 --min-size 1024
```

```
Scanning: ./projects
Found 3 duplicate group(s):

[Group 1] Size: 4.20 MB | Hash: 8f85a5c3...
  Reclaimable: 8.40 MB (2 redundant copies)
  - ./projects/backup/report_final.pdf
  - ./projects/archive/report_final.pdf

Total Reclaimable Space: 48.07 MB
```

---

#### `report` — Disk Usage Analysis

```bash
dedupe report <DIRECTORY> [OPTIONS]

Options:
  --top <N>           Top N largest files (default: 10)
  --stale-months <N>  Flag files older than N months (default: 6)
  --json              Emit machine-readable JSON
```

**Example:**

```bash
dedupe report ./downloads --top 20 --stale-months 3
```

```
=== DISK USAGE AUDIT REPORT ===
Directory: ./downloads | Total files: 1,284

--- Top 3 Largest Files ---
   4.20 MB  ./downloads/ubuntu-24.04.iso
   1.80 MB  ./downloads/backup.zip
    512 KB  ./downloads/dataset.csv

--- Top Directories by Size ---
   6.72 GB (820 files)  ./downloads/videos

--- Extension Breakdown ---
  .mp4   312 files   6.40 GB
  .zip    48 files   1.20 GB

--- Hygiene Audit ---
  Empty files: 3  |  Empty dirs: 7  |  Stale (>3mo): 92
```

---

#### `snapshot` — Point-in-Time Index

Creates a cryptographic index of a directory. Each file is recorded with its path, size, and BLAKE3 hash.

```bash
dedupe snapshot <DIRECTORY> -o <OUTPUT.json>
```

---

#### `diff` — Differential Audit

Compares two snapshots to detect added, removed, modified, and moved/renamed files.

```bash
dedupe diff <OLD.json> <NEW.json> [--json]
```

**Example:**

```bash
dedupe snapshot ./src -o baseline.json
# ... make changes ...
dedupe snapshot ./src -o updated.json
dedupe diff baseline.json updated.json
```

```
=== SNAPSHOT DIFF ===
Unchanged: 42 files

Added (1):    + ./src/new_feature.rs
Removed (1):  - ./src/deprecated.rs
Modified (2): * ./src/main.rs  * Cargo.toml
Moved (1):    >>> ./src/old_name.rs --> ./src/pool.rs
```

---

## Design Decisions

### Why BLAKE3 over MD5 / SHA-256?

- **Speed:** 10–15x faster than SHA-256 on modern SIMD hardware
- **Security:** Cryptographically collision-resistant (unlike MD5/SHA-1)
- **Parallelism:** Tree-hashing structure scales with available CPU cores

### Why Streaming 64 KB Chunks instead of `fs::read`?

Loading full files via `fs::read` allocates RAM equal to file size per worker. A single 20 GB file would exhaust system memory. Fixed 64 KB `BufReader` buffers bound memory to `O(workers × 64 KB)` — under 1 MB even at 16 threads.

### Why a Custom Thread Pool instead of Rayon?

A custom pool using `std::thread::spawn + mpsc + Arc<Mutex<Receiver<HashJob>>>` provides explicit control over job dispatch ordering, thread lifecycle, graceful shutdown, and back-pressure — compared to rayon's opaque work-stealing scheduler.

### Move Detection in Diff Engine

Files are identified as "moved" when an added path and a removed path share the same BLAKE3 hash and byte length. The diff engine maintains per-hash candidate sets and performs 1-to-1 pairings to prevent false positives when multiple identical copies exist across both snapshots.

---

## Limitations

| Limitation | Notes |
|---|---|
| Symlinks / Junctions | Skipped via `symlink_metadata` to prevent filesystem cycles |
| Hardlinks | Files sharing the same inode are not grouped as a unit |
| Very Large Trees | In-memory `Vec<(PathBuf, u64)>` walk; 50M+ file trees may need streaming |
| Network Drives | Performance degrades on high-latency network filesystems |

---

## Contributing

Contributions and bug reports are welcome!

```bash
git clone https://github.com/ShashwatGohel/dedupe.git
cd dedupe
cargo test
cargo clippy
cargo fmt
```

Please open an issue before submitting a large PR to align on the approach.

---

## License

This project is licensed under the **MIT License**.

---

<div align="center">

Built with Rust · BLAKE3 · zero external runtime dependencies

Developed with pair-programming assistance from [Antigravity AI](https://antigravity.dev)

</div>


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
