use std::{
    borrow::Cow,
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use open_download_manager_lib::{
    domain::{
        download::{redact_download_url, DownloadRecord, DownloadStatus, SerializableDownloadRecordRef},
        snapshot::DownloadSummary,
    },
    infrastructure::file_icons::is_safe_extension,
};

fn generate_mock_records(count: usize) -> Vec<DownloadRecord> {
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        records.push(DownloadRecord {
            id: format!("dl_{i}"),
            url: format!("https://example.com/downloads/file_{i}.zip?token=secret_{i}&auth=xyz"),
            filename: format!("file_{i}.zip"),
            status: if i % 10 == 0 {
                DownloadStatus::Completed
            } else if i % 3 == 0 {
                DownloadStatus::Downloading
            } else {
                DownloadStatus::Queued
            },
            downloaded: (i as u64) * 1024,
            total: Some((i as u64) * 2048 + 1024),
            speed: if i % 3 == 0 { 1024 * 1024 } else { 0 },
            created_at: format!("2026-11-0{}T12:00:00Z", (i % 28) + 1),
            needs_review: i % 15 == 0,
            scheduled_at: None,
            description: Some(format!("Description for record {i}")),
            category: None,
            directory: Some(format!("/Users/devansh/Downloads/folder_{}", i % 5)),
            path: Some(format!("/Users/devansh/Downloads/folder_{}/file_{i}.zip", i % 5)),
            last_try: None,
            error: None,
            notice: None,
            estimated: false,
            transfer_kind: None,
            kind: None,
            resumable: Some(true),
            duration: Some(120.0),
            connections: 4,
            connections_progress: Vec::new(),
            audio_url: None,
            sabr: None,
            referer: None,
            youtube_format: None,
        });
    }
    records
}

fn time_fn<F: FnMut() -> R, R>(warmup: usize, iterations: usize, mut f: F) -> Duration {
    for _ in 0..warmup {
        let _ = std::hint::black_box(f());
    }
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = std::hint::black_box(f());
    }
    start.elapsed() / (iterations as u32)
}

fn format_duration(d: Duration) -> String {
    if d.as_secs() > 0 {
        format!("{:.3} s", d.as_secs_f64())
    } else if d.as_millis() > 0 {
        format!("{:.2} ms", d.as_secs_f64() * 1000.0)
    } else if d.as_micros() > 0 {
        format!("{:.1} µs", d.as_secs_f64() * 1_000_000.0)
    } else {
        format!("{} ns", d.as_nanos())
    }
}

fn main() {
    println!("===============================================================================");
    println!("          HIGH-PERFORMANCE RUST BENCHMARKS: BASELINE (main) vs OPTIMIZED       ");
    println!("===============================================================================\n");

    // -------------------------------------------------------------------------
    // Benchmark 1: Query & Filtering (The Eager Clone vs Borrowed Inspection)
    // -------------------------------------------------------------------------
    println!("--- Benchmark 1: State Inspection & Filtering (Event Publisher / Query) ---");
    println!("Description: Looking up a record by ID from shared in-memory history state.");
    for &count in &[1_000, 10_000, 20_000] {
        let records = generate_mock_records(count);
        let shared = Arc::new(Mutex::new(records));
        let target_id = format!("dl_{}", count / 2);

        // Baseline (main): clones entire vector from mutex on every inspection
        let baseline_time = time_fn(3, 10, || {
            let list = shared.lock().unwrap().clone();
            let found = list.into_iter().find(|d| d.id == target_id);
            found
        });

        // Optimized (perf branch): with_downloads zero-allocation borrowed inspection
        let optimized_time = time_fn(3, 10, || {
            let guard = shared.lock().unwrap();
            let found = guard.iter().find(|d| d.id == target_id).cloned();
            found
        });

        let speedup = baseline_time.as_secs_f64() / optimized_time.as_secs_f64();
        println!(
            "  [{:5} records] Baseline: {:>10} | Optimized: {:>10} | Speedup: {:>6.1}x",
            count,
            format_duration(baseline_time),
            format_duration(optimized_time),
            speedup
        );
    }
    println!();

    // -------------------------------------------------------------------------
    // Benchmark 2: History Serialization (save_history)
    // -------------------------------------------------------------------------
    println!("--- Benchmark 2: History Disk Serialization (save_history) ---");
    println!("Description: Redacting URLs and serializing download history to JSON.");
    for &count in &[1_000, 5_000, 10_000] {
        let records = generate_mock_records(count);

        // Baseline (main): clones every DownloadRecord upfront
        let baseline_time = time_fn(2, 5, || {
            let cloned: Vec<DownloadRecord> = records
                .iter()
                .cloned()
                .map(|mut d| {
                    d.url = redact_download_url(&d.url);
                    d
                })
                .collect();
            serde_json::to_vec(&cloned).unwrap()
        });

        // Optimized (perf branch): zero-copy borrowed reference view
        let optimized_time = time_fn(2, 5, || {
            let borrowed: Vec<SerializableDownloadRecordRef> = records
                .iter()
                .map(|d| d.to_serialized_ref(redact_download_url(&d.url)))
                .collect();
            serde_json::to_vec(&borrowed).unwrap()
        });

        let speedup = baseline_time.as_secs_f64() / optimized_time.as_secs_f64();
        println!(
            "  [{:5} records] Baseline: {:>10} | Optimized: {:>10} | Speedup: {:>6.1}x",
            count,
            format_duration(baseline_time),
            format_duration(optimized_time),
            speedup
        );
    }
    println!();

    // -------------------------------------------------------------------------
    // Benchmark 3: Snapshot Generation (UI Fetch)
    // -------------------------------------------------------------------------
    println!("--- Benchmark 3: Snapshot Construction (UI Fetch Command) ---");
    println!("Description: Constructing DownloadSummary objects for Tauri IPC.");
    for &count in &[1_000, 10_000] {
        let records = generate_mock_records(count);

        // Baseline (main): clones entire Vec<DownloadRecord> before mapping to summary
        let baseline_time = time_fn(2, 5, || {
            let cloned_records = records.clone();
            let summaries: Vec<DownloadSummary> = cloned_records
                .into_iter()
                .map(DownloadSummary::from)
                .collect();
            summaries
        });

        // Optimized (perf branch): direct construction from borrowed &[DownloadRecord]
        let optimized_time = time_fn(2, 5, || {
            let summaries: Vec<DownloadSummary> = records
                .iter()
                .map(DownloadSummary::from)
                .collect();
            summaries
        });

        let speedup = baseline_time.as_secs_f64() / optimized_time.as_secs_f64();
        println!(
            "  [{:5} records] Baseline: {:>10} | Optimized: {:>10} | Speedup: {:>6.1}x",
            count,
            format_duration(baseline_time),
            format_duration(optimized_time),
            speedup
        );
    }
    println!();

    // -------------------------------------------------------------------------
    // Benchmark 4: Path Normalization & Extension Deduplication
    // -------------------------------------------------------------------------
    println!("--- Benchmark 4: Path Normalization & Extension Deduplication ---");
    println!("Description: Processing 10,000 file paths for icon detection.");
    {
        let paths: Vec<String> = (0..10_000)
            .map(|i| {
                let ext = match i % 8 {
                    0 => "mp4",
                    1 => "zip",
                    2 => "pdf",
                    3 => "exe",
                    4 => "png",
                    5 => "tar.gz",
                    6 => "rs",
                    _ => "txt",
                };
                format!("/Users/devansh/Downloads/folder_{i}/archive_name_{i}.{ext}")
            })
            .collect();

        // Baseline (main): .replace('\\', "/") always allocating String + Vec::contains (O(N^2))
        let baseline_time = time_fn(2, 10, || {
            let mut extensions = Vec::new();
            for p in &paths {
                let normalized = p.replace('\\', "/");
                let path = Path::new(&normalized);
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let candidate = format!(".{}", ext.to_ascii_lowercase());
                    if is_safe_extension(&candidate) && !extensions.contains(&candidate) {
                        extensions.push(candidate);
                    }
                }
            }
            extensions
        });

        // Optimized (perf branch): Cow zero-allocation + HashSet (O(N))
        let optimized_time = time_fn(2, 10, || {
            let mut extensions = HashSet::new();
            for p in &paths {
                let normalized = if p.contains('\\') {
                    Cow::Owned(p.replace('\\', "/"))
                } else {
                    Cow::Borrowed(p.as_str())
                };
                let path = Path::new(normalized.as_ref());
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let candidate = format!(".{}", ext.to_ascii_lowercase());
                    if is_safe_extension(&candidate) {
                        extensions.insert(candidate);
                    }
                }
            }
            extensions
        });

        let speedup = baseline_time.as_secs_f64() / optimized_time.as_secs_f64();
        println!(
            "  [10,000 paths] Baseline: {:>10} | Optimized: {:>10} | Speedup: {:>6.1}x",
            format_duration(baseline_time),
            format_duration(optimized_time),
            speedup
        );
    }
    println!();

    // -------------------------------------------------------------------------
    // Benchmark 5: Original "Fix Your Rust Functions" Scaling Test
    // -------------------------------------------------------------------------
    println!("--- Benchmark 5: The Original Scaling Test (v1 vs v2 vs v3 vs v4) ---");
    println!("Description: Filtering matching records and extracting the top 10.");
    for &count in &[10_000, 20_000] {
        let records = generate_mock_records(count);
        let limit = 10;

        // v1: Eager clone on filter, owned sort, truncate
        let v1_time = time_fn(2, 5, || {
            let mut matches: Vec<DownloadRecord> = records
                .iter()
                .filter(|r| r.status == DownloadStatus::Downloading)
                .cloned()
                .collect();
            matches.sort_by(|a, b| b.downloaded.cmp(&a.downloaded));
            matches.truncate(limit);
            matches
        });

        // v2: Deferred clone, sort pointers, truncate, clone top limit
        let v2_time = time_fn(2, 5, || {
            let mut matches: Vec<&DownloadRecord> = records
                .iter()
                .filter(|r| r.status == DownloadStatus::Downloading)
                .collect();
            matches.sort_by(|a, b| b.downloaded.cmp(&a.downloaded));
            matches.truncate(limit);
            let result: Vec<DownloadRecord> = matches.into_iter().cloned().collect();
            result
        });

        // v3: Zero-copy lifetimes (Vec<&Record>)
        let v3_time = time_fn(2, 5, || {
            let mut matches: Vec<&DownloadRecord> = records
                .iter()
                .filter(|r| r.status == DownloadStatus::Downloading)
                .collect();
            matches.sort_by(|a, b| b.downloaded.cmp(&a.downloaded));
            matches.truncate(limit);
            matches
        });

        // v4: Introselect (select_nth_unstable_by)
        let v4_time = time_fn(2, 5, || {
            let mut matches: Vec<&DownloadRecord> = records
                .iter()
                .filter(|r| r.status == DownloadStatus::Downloading)
                .collect();
            if matches.len() > limit {
                matches.select_nth_unstable_by(limit, |a, b| b.downloaded.cmp(&a.downloaded));
                matches.truncate(limit);
            }
            matches.sort_unstable_by(|a, b| b.downloaded.cmp(&a.downloaded));
            matches
        });

        println!("  [Count: {:5}]", count);
        println!("    v1 (Eager clone):               {:>10}", format_duration(v1_time));
        println!("    v2 (Deferred clone):            {:>10} ({:.1}x vs v1)", format_duration(v2_time), v1_time.as_secs_f64() / v2_time.as_secs_f64());
        println!("    v3 (Zero-Copy Lifetimes):       {:>10} ({:.1}x vs v1)", format_duration(v3_time), v1_time.as_secs_f64() / v3_time.as_secs_f64());
        println!("    v4 (Introselect + Zero-Copy):   {:>10} ({:.1}x vs v1)", format_duration(v4_time), v1_time.as_secs_f64() / v4_time.as_secs_f64());
    }
    println!("\n===============================================================================");
}
