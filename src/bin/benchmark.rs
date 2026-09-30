use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use dedupe::{pool, report, walk};

fn create_benchmark_dataset(root: &Path) {
    println!("Generating benchmark dataset at: {}", root.display());
    fs::create_dir_all(root).unwrap();

    let sub1 = root.join("dir_a");
    let sub2 = root.join("dir_b");
    let sub3 = root.join("dir_c/nested");
    fs::create_dir_all(&sub1).unwrap();
    fs::create_dir_all(&sub2).unwrap();
    fs::create_dir_all(&sub3).unwrap();

    // 1. Generate 1,500 unique files with strictly UNIQUE sizes (e.g. 100 + i bytes)
    for i in 0..1500 {
        let dir = if i % 3 == 0 {
            &sub1
        } else if i % 3 == 1 {
            &sub2
        } else {
            &sub3
        };
        let file_path = dir.join(format!("unique_{}.txt", i));
        // Every unique file has a distinct byte length
        let content = vec![(i % 251) as u8; 100 + i];
        let mut f = File::create(file_path).unwrap();
        f.write_all(&content).unwrap();
    }

    // 2. Generate 15 duplicate groups (each with 5 to 10 copies)
    for g in 0..15 {
        let size = 1024 * (g + 1); // 1KB, 2KB, ..., 15KB
        let payload = vec![(g * 17 % 255) as u8; size];

        for copy in 0..6 {
            let dir = if copy % 3 == 0 {
                &sub1
            } else if copy % 3 == 1 {
                &sub2
            } else {
                &sub3
            };
            let file_path = dir.join(format!("dupe_group_{}_copy_{}.dat", g, copy));
            let mut f = File::create(file_path).unwrap();
            f.write_all(&payload).unwrap();
        }
    }

    // 3. Generate large duplicate files (500 KB to 2 MB)
    for lg in 0..3 {
        let size = 512 * 1024 * (lg + 1); // 512KB, 1MB, 1.5MB
        let payload = vec![0xFE; size];

        for copy in 0..4 {
            let file_path = sub3.join(format!("large_group_{}_copy_{}.bin", lg, copy));
            let mut f = File::create(file_path).unwrap();
            f.write_all(&payload).unwrap();
        }
    }

    println!("Dataset generated successfully.");
}

fn main() {
    let bench_dir = std::env::temp_dir().join("dedupe_benchmark_dataset");
    if bench_dir.exists() {
        let _ = fs::remove_dir_all(&bench_dir);
    }

    create_benchmark_dataset(&bench_dir);

    println!("\n=======================================================");
    println!("               DEDUPE BENCHMARK SUITE");
    println!("=======================================================\n");

    let scanned = walk::walk(&bench_dir);
    let total_bytes: u64 = scanned.iter().map(|(_, s)| *s).sum();
    println!("Total files in benchmark set: {}", scanned.len());
    println!("Total volume: {}", report::format_bytes(total_bytes));

    // Experiment 1: Size-filtered vs Unfiltered Hashing
    println!("\n--- Experiment 1: Size Filter vs No Filter ---");

    // A) No size filter: hash EVERY single file
    let start_unfiltered = Instant::now();
    let mut unfiltered_candidates: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (p, s) in &scanned {
        unfiltered_candidates.entry(*s).or_default().push(p.clone());
    }
    // We treat all files as needing hashing
    let mut all_files_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    all_files_map.insert(0, scanned.iter().map(|(p, _)| p.clone()).collect());
    // Hash using 4 threads
    let pool4 = pool::WorkerPool::new(4);
    let _ = pool4.hash_candidates_parallel(&all_files_map);
    let dur_unfiltered = start_unfiltered.elapsed();
    let unfiltered_count = scanned.len();

    // B) With size filter:
    let start_filtered = Instant::now();
    let size_groups = walk::group_by_size(scanned.clone());
    let filtered_candidates = walk::filter_candidate_groups(size_groups);
    let filtered_count: usize = filtered_candidates.values().map(|v| v.len()).sum();
    let dupes = pool4.hash_candidates_parallel(&filtered_candidates);
    let dur_filtered = start_filtered.elapsed();

    println!("Unfiltered: Hashed {} files in {:?}", unfiltered_count, dur_unfiltered);
    println!("Filtered:   Hashed {} files in {:?}", filtered_count, dur_filtered);
    let files_avoided = unfiltered_count.saturating_sub(filtered_count);
    let pct_avoided = (files_avoided as f64 / unfiltered_count as f64) * 100.0;
    println!("Hashing avoided for {} files ({:.1}% of dataset)!\n", files_avoided, pct_avoided);

    // Experiment 2: Thread Scaling
    println!("--- Experiment 2: Multi-threading Scaling (1, 2, 4, 8, 12 threads) ---");
    let thread_configs = [1, 2, 4, 8, 12];
    println!("| Threads | Time (ms) | Speedup vs 1T | Duplicate Groups |");
    println!("|---------|-----------|---------------|------------------|");

    let mut baseline_ms = 0.0;

    for &t in &thread_configs {
        // Run 3 iterations and take median / average
        let mut runs = Vec::new();
        for _ in 0..3 {
            let start = Instant::now();
            let _ = pool::WorkerPool::new(t).hash_candidates_parallel(&filtered_candidates);
            runs.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_ms = runs[1]; // median of 3

        if t == 1 {
            baseline_ms = median_ms;
        }

        let speedup = if baseline_ms > 0.0 {
            baseline_ms / median_ms
        } else {
            1.0
        };

        println!(
            "| {:>7} | {:>9.2} | {:>12.2}x | {:>16} |",
            t, median_ms, speedup, dupes.len()
        );
    }

    println!("\nCleaning up benchmark files...");
    let _ = fs::remove_dir_all(&bench_dir);
    println!("Benchmark run complete.");
}
