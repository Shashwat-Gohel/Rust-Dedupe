use std::env;
use std::path::PathBuf;

mod hash;
mod walk;

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from(".")
    };

    println!("Scanning directory: {}", target_dir.display());

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

    println!("\nHashing candidate files...");
    let duplicate_groups = hash::find_duplicates(&duplicate_candidates);

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
