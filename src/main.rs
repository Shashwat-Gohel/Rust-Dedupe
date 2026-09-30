use std::env;
use std::path::PathBuf;

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

    println!(
        "\nSize groups with potential duplicates (>= 2 files): {}",
        duplicate_candidates.len()
    );

    for (size, paths) in &duplicate_candidates {
        println!("\n[Size: {} bytes] ({} files)", size, paths.len());
        for p in paths {
            println!("  - {}", p.display());
        }
    }
}
