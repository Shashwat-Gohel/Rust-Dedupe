# dedupe: Technical Interview Preparation & Deep-Dive Guide

This document contains in-depth, interview-ready answers to the core systems programming and Rust architecture questions from Phase 10 of `todo.md`.

---

### 1. Why group by size first? What did it save in our benchmark?
- **Concept:** Two files cannot be duplicates unless they have the exact same size in bytes. Querying file size comes "for free" during directory traversal (`fs::symlink_metadata` reads the inode/directory record without reading file payload blocks).
- **Benchmark Proof:** In our benchmark of 1,602 files, 1,499 files had unique sizes. By filtering candidates before hashing, we bypassed hashing for **93.6% of the dataset**, slashing execution time from **68.48 ms down to 7.65 ms (~8.95x speedup)**.
- **Key Takeaway:** Always use cheap metadata filters to prune search spaces before performing expensive I/O operations.

---

### 2. Why chunked hashing? What happens with a 20 GB file?
- **Concept:** `fs::read` or `read_to_end` allocates a contiguous buffer in RAM large enough to hold the entire file content. For a 20 GB file, this would demand 20 GB of resident physical memory. Across 8 concurrent worker threads, this would require 160 GB of RAM, triggering system out-of-memory (OOM) crashes.
- **Solution:** We process files in fixed **64 KB chunks** (`BufReader` + `blake3::Hasher`). The hasher processes the 64 KB block, updates its internal Merkle tree state, and the buffer is overwritten in the next iteration.
- **Memory Footprint:** RAM consumption is strictly $O(1)$ per thread ($\approx 64\text{ KB}$ per thread, or $<1\text{ MB}$ total across 12 threads), regardless of whether files are 10 KB or 500 GB.

---

### 3. Hash collisions: How likely are they, and when would you byte-compare?
- **Probability:** BLAKE3 produces a 256-bit cryptographic digest ($2^{256} \approx 1.15 \times 10^{77}$ combinations). According to the birthday paradox, you would need to hash roughly $2^{128} \approx 3.4 \times 10^{38}$ distinct files of the exact same size before having a 50% probability of a single collision.
- **Comparison:** This is billions of times smaller than the probability of an undetected hardware bit-flip caused by cosmic rays in server RAM.
- **When to Byte-Compare:**
  - If using non-cryptographic hashes (CRC32, FNV-1a, Murmur3, xxHash), byte-by-byte comparison is mandatory.
  - When writing destructive cleanup actions (e.g. `--delete-duplicates`), implementing a final streaming byte-by-byte verification pass between candidate pairs is a best practice for mission-critical disaster prevention.

---

### 4. Why is thread speedup limited? (I/O-bound vs CPU-bound)
- **Concept:**
  - *CPU-bound tasks* (e.g., pure in-memory encryption, matrix multiplication) scale almost linearly with CPU core counts ($N$ cores $\rightarrow \approx N\text{x}$ speedup).
  - *I/O-bound tasks* (reading files from disk) are constrained by disk read queues, OS file descriptor locks, page cache synchronization, and physical storage bus saturation.
- **Benchmark Proof:**
  - 1 Thread: 13.21 ms
  - 2 Threads: 10.23 ms (1.29x)
  - 4 Threads: 6.69 ms (1.98x)
  - 8 Threads: 4.89 ms (2.70x)
  - 12 Threads: 5.22 ms (speedup plateaued / degraded)
- **Takeaway:** Beyond 8 threads on a standard SSD/NVMe, worker threads spend more time waiting on kernel I/O scheduling and contending on the `Mutex<Receiver>` queue than doing useful hashing.

---

### 5. How does Rust prevent data races in our worker pool?
- **`Send` and `Sync` traits:**
  - Types transferred across thread boundaries must implement `Send`.
  - Types shared by reference across threads must implement `Sync`.
- **Why `mpsc::Receiver` is `!Sync`:** Standard `mpsc::Receiver` cannot be referenced by multiple threads simultaneously because its internal queue mutation is not thread-safe.
- **Why `Arc<Mutex<Receiver<T>>>`:**
  - `Arc` provides atomic reference counting so multiple threads can own the shared queue.
  - `Mutex` guarantees mutual exclusion: only one worker thread can call `.recv()` at any instant, satisfying the compiler's safety invariants.
  - Rust guarantees at compile time that no two threads can mutate or access the receiver concurrently without holding the mutex lock.
- **Graceful Shutdown:** When the main thread drops the primary `Sender`, the channel closes. Any worker calling `lock.recv()` receives `Err(RecvError)` and breaks from the loop, cleanly shutting down all OS threads.

---

### 6. How does move detection work and where does it fail?
- **Mechanism:** When comparing `Snapshot A` and `Snapshot B`:
  1. Files present only in A form the `candidate_removed` list.
  2. Files present only in B form the `candidate_added` list.
  3. If an entry in `candidate_removed` has the same content hash and size as an entry in `candidate_added`, it is classified as a `Moved / Renamed` file (`old_path -> new_path`).
- **Where it fails (The Duplicate Edge Case):**
  - If a project contained two identical copies (`copy1.txt` and `copy2.txt`) in snapshot A, and `copy1.txt` was deleted while `copy2.txt` remained and a new duplicate `sub/copy3.txt` was added.
  - Without tracking hash sets, naive lookup could report `copy1.txt -> sub/copy3.txt` as a move, or match multiple times against the same file.
  - **Our Fix:** We group candidate removals and additions into a `HashMap<Hash, Vec<FileEntry>>` and pop entries 1-to-1. Excess removals stay as true deletions; excess additions stay as true creations.

---

### 7. How would you scale this for massive filesystems (50+ million files)?
1. **Multi-Stage Hashing (Partial Hashing):**
   - For files larger than 1 MB, hash only the first **4 KB header**.
   - If the first 4 KB hashes match, hash the trailing **4 KB footer**.
   - Only compute the full cryptographic hash if both header and footer match.
2. **Persistent Hash Cache by `(size, mtime, inode)`:**
   - Store past hashes in an embedded KV database (like `sled` or `redb`).
   - If a file's `size` and `mtime` haven't changed, reuse the cached hash without reading the file.
3. **Hardlink / Inode Awareness:**
   - On Unix (`st_ino`, `st_dev`) and Windows (`FileIndex`), files pointing to the exact same inode share disk blocks on the filesystem layer. Grouping them as separate duplicates overcounts reclaimable space.
4. **Streaming Traversal:**
   - Replace in-memory `Vec<PathBuf>` with an iterator pipeline or channel producer to keep memory flat even when scanning tens of millions of files.

---

### 8. What would you do differently with more time?
- Implement an interactive terminal TUI (using `ratatui`) to allow users to navigate duplicate groups and mark files for automated symlinking, hardlinking, or deletion.
- Add `--dry-run` and safe atomic deletion with trash/recycle bin integration.
- Add support for inode/device deduplication on Unix and reparse point file ID tracking on Windows.
- Provide a progress bar using `indicatif` to show real-time scan and hash rates (MB/s).
