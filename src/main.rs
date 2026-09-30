use std::env;
use std::path::PathBuf;

mod hash;
mod pool;
mod walk;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut target_dir = PathBuf::from(".");
    let mut num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--threads" || args[i] == "-t") && i + 1 < args.len() {
            if let Ok(val) = args[i + 1].parse::<usize>() {
                num_threads = val;
            }
            i += 2;
        } else if !args[i].starts_with('-') {
            target_dir = PathBuf::from(&args[i]);
            i += 1;
        } else {
            i += 1;
        }
    }

    println!("Scanning directory: {}", target_dir.display());
    println!("Worker threads: {}", num_threads);

    let scanned_files = walk::walk(&target_dir);
    println!("Total files found (excluding symlinks): {}", scanned_files.len());

    let size_groups = walk::group_by_size(scanned_files);
    let duplicate_candidates = walk::filter_candidate_groups(size_groups);

    let candidate_file_count: usize = duplicate_candidates.values().map(|v| v.len()).sum();
    println!(
        "Candidate groups: {} ({} files to hash)",
        duplicate_candidates.len(),
        candidate_file_count
    );

    println!("\nHashing candidate files in parallel...");
    let pool = pool::WorkerPool::new(num_threads);
    let duplicate_groups = pool.hash_candidates_parallel(&duplicate_candidates);

    println!(
        "\nFound {} duplicate group(s):",
        duplicate_groups.len()
    );

    for (i, group) in duplicate_groups.iter().enumerate() {
        println!(
            "\n[Group {}] Size: {} bytes | Hash: {}",
            i + 1,
            group.file_size,
            group.hash.to_hex()
        );
        println!(
            "  Reclaimable: {} bytes ({} copies)",
            group.reclaimable_bytes(),
            group.files.len() - 1
        );
        for p in &group.files {
            println!("  - {}", p.display());
        }
    }

    let total_reclaimable = hash::total_reclaimable_space(&duplicate_groups);
    println!(
        "\nTotal reclaimable space: {} bytes (~{:.2} KB)",
        total_reclaimable,
        total_reclaimable as f64 / 1024.0
    );
}
