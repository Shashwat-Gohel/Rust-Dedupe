use std::path::PathBuf;
use std::process;
use std::time::Duration;

use clap::{Parser, Subcommand};

mod hash;
mod pool;
mod report;
mod snapshot;
mod walk;

#[derive(Parser, Debug)]
#[command(
    name = "dedupe",
    author = "Dedupe Team",
    version = "0.1.0",
    about = "High-performance disk audit and deduplication CLI tool written in Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan a directory for duplicate files using parallel chunked hashing
    Scan {
        /// Target directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Number of worker threads for parallel hashing
        #[arg(short, long)]
        threads: Option<usize>,

        /// Minimum file size in bytes to consider
        #[arg(long, default_value_t = 1)]
        min_size: u64,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate disk usage reports (largest files, extension breakdown, folders)
    Report {
        /// Target directory to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Number of largest files to display
        #[arg(long, default_value_t = 10)]
        top: usize,

        /// Flag files older than N months as stale
        #[arg(long, default_value_t = 6)]
        stale_months: u64,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Create a point-in-time cryptographic snapshot of a directory
    Snapshot {
        /// Directory to snapshot
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output JSON snapshot destination path
        #[arg(short, long, default_value = "snap.json")]
        output: PathBuf,
    },

    /// Compare two snapshots to detect added, removed, modified, and moved files
    Diff {
        /// Path to baseline (old) snapshot JSON
        old: PathBuf,

        /// Path to target (new) snapshot JSON
        new: PathBuf,

        /// Output diff report as JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            threads,
            min_size,
            json,
        } => run_scan(path, threads, min_size, json),
        Commands::Report {
            path,
            top,
            stale_months,
            json,
        } => run_report(path, top, stale_months, json),
        Commands::Snapshot { path, output } => run_snapshot(path, output),
        Commands::Diff { old, new, json } => run_diff(old, new, json),
    }
}

fn run_scan(path: PathBuf, threads: Option<usize>, min_size: u64, json_output: bool) {
    if !path.exists() {
        eprintln!("[ERROR] Path '{}' does not exist.", path.display());
        process::exit(1);
    }

    let num_threads = threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    });

    if !json_output {
        println!("Scanning: {}", path.display());
        println!("Worker threads: {}", num_threads);
        println!("Minimum file size: {} bytes", min_size);
    }

    let scanned_files = walk::walk(&path);
    let filtered_files: Vec<(PathBuf, u64)> = scanned_files
        .into_iter()
        .filter(|(_, size)| *size >= min_size)
        .collect();

    let size_groups = walk::group_by_size(filtered_files);
    let duplicate_candidates = walk::filter_candidate_groups(size_groups);

    let candidate_file_count: usize = duplicate_candidates.values().map(|v| v.len()).sum();
    if !json_output {
        println!(
            "Potential duplicate candidates: {} groups ({} files to hash)",
            duplicate_candidates.len(),
            candidate_file_count
        );
        println!("Hashing candidate files...");
    }

    let pool = pool::WorkerPool::new(num_threads);
    let duplicate_groups = pool.hash_candidates_parallel(&duplicate_candidates);
    let total_reclaimable = hash::total_reclaimable_space(&duplicate_groups);

    if json_output {
        #[derive(serde::Serialize)]
        struct ScanJsonResult<'a> {
            duplicate_groups: &'a [hash::DuplicateGroup],
            total_reclaimable_bytes: u64,
            human_reclaimable: String,
        }

        let out = ScanJsonResult {
            duplicate_groups: &duplicate_groups,
            total_reclaimable_bytes: total_reclaimable,
            human_reclaimable: report::format_bytes(total_reclaimable),
        };
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("\nFound {} duplicate group(s):", duplicate_groups.len());

        for (i, group) in duplicate_groups.iter().enumerate() {
            println!(
                "\n[Group {}] Size: {} ({}) | Hash: {}",
                i + 1,
                group.file_size,
                report::format_bytes(group.file_size),
                group.hash.to_hex()
            );
            println!(
                "  Reclaimable: {} ({} redundant copies)",
                report::format_bytes(group.reclaimable_bytes()),
                group.files.len() - 1
            );
            for p in &group.files {
                println!("  - {}", p.display());
            }
        }

        println!(
            "\nTotal Reclaimable Space: {}",
            report::format_bytes(total_reclaimable)
        );
    }
}

fn run_report(path: PathBuf, top: usize, stale_months: u64, json_output: bool) {
    if !path.exists() {
        eprintln!("[ERROR] Path '{}' does not exist.", path.display());
        process::exit(1);
    }

    let scanned_files = walk::walk(&path);
    let top_files = report::top_largest_files(&scanned_files, top);
    let dir_stats = report::directory_size_aggregation(&scanned_files, 10);
    let ext_stats = report::extension_breakdown(&scanned_files);

    let stale_duration = Duration::from_secs(stale_months * 30 * 24 * 3600);
    let audit = report::audit_storage(&path, stale_duration);

    if json_output {
        #[derive(serde::Serialize)]
        struct ReportJson<'a> {
            total_files: usize,
            top_files: &'a [(PathBuf, u64)],
            top_directories: &'a [report::DirStat],
            extensions: &'a [report::ExtensionStat],
            empty_files_count: usize,
            empty_dirs_count: usize,
            stale_files_count: usize,
        }

        let out = ReportJson {
            total_files: scanned_files.len(),
            top_files: &top_files,
            top_directories: &dir_stats,
            extensions: &ext_stats,
            empty_files_count: audit.empty_files.len(),
            empty_dirs_count: audit.empty_dirs.len(),
            stale_files_count: audit.stale_files.len(),
        };
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("=== DISK USAGE AUDIT REPORT ===");
        println!("Directory: {}", path.display());
        println!("Total regular files: {}\n", scanned_files.len());

        println!("--- Top {} Largest Files ---", top_files.len());
        for (p, size) in &top_files {
            println!("  {:>10}  {}", report::format_bytes(*size), p.display());
        }

        println!("\n--- Top Folders by Storage ---");
        for ds in &dir_stats {
            println!(
                "  {:>10} ({} files)  {}",
                report::format_bytes(ds.total_bytes),
                ds.file_count,
                ds.path.display()
            );
        }

        println!("\n--- Breakdown by Extension ---");
        for es in &ext_stats {
            println!(
                "  .{:<8} {:>6} files  {:>10}",
                es.ext,
                es.count,
                report::format_bytes(es.total_bytes)
            );
        }

        println!("\n--- Hygiene Audit ---");
        println!("  Empty files: {}", audit.empty_files.len());
        println!("  Empty directories: {}", audit.empty_dirs.len());
        println!(
            "  Stale files (> {} months): {}",
            stale_months,
            audit.stale_files.len()
        );
    }
}

fn run_snapshot(path: PathBuf, output: PathBuf) {
    if !path.exists() {
        eprintln!("[ERROR] Path '{}' does not exist.", path.display());
        process::exit(1);
    }

    println!("Creating snapshot of: {}", path.display());
    match snapshot::Snapshot::create(&path) {
        Ok(snap) => {
            let count = snap.files.len();
            if let Err(err) = snap.save_to_file(&output) {
                eprintln!("[ERROR] Failed to write snapshot to '{}': {}", output.display(), err);
                process::exit(1);
            }
            println!(
                "Successfully indexed {} files to snapshot: {}",
                count,
                output.display()
            );
        }
        Err(err) => {
            eprintln!("[ERROR] Snapshot failed: {}", err);
            process::exit(1);
        }
    }
}

fn run_diff(old_path: PathBuf, new_path: PathBuf, json_output: bool) {
    let old_snap = match snapshot::Snapshot::load_from_file(&old_path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("[ERROR] Cannot read old snapshot '{}': {}", old_path.display(), err);
            process::exit(1);
        }
    };

    let new_snap = match snapshot::Snapshot::load_from_file(&new_path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("[ERROR] Cannot read new snapshot '{}': {}", new_path.display(), err);
            process::exit(1);
        }
    };

    let diff = old_snap.diff(&new_snap);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff).unwrap());
    } else {
        println!("=== SNAPSHOT DIFF ===");
        println!("Old: {}", old_path.display());
        println!("New: {}", new_path.display());
        println!("Unchanged files: {}", diff.unchanged_count);

        println!("\nAdded ({}):", diff.added.len());
        for p in &diff.added {
            println!("  + {}", p.display());
        }

        println!("\nRemoved ({}):", diff.removed.len());
        for p in &diff.removed {
            println!("  - {}", p.display());
        }

        println!("\nModified ({}):", diff.modified.len());
        for p in &diff.modified {
            println!("  * {}", p.display());
        }

        println!("\nMoved / Renamed ({}):", diff.moved.len());
        for (old_p, new_p) in &diff.moved {
            println!("  > {} -> {}", old_p.display(), new_p.display());
        }
    }
}
