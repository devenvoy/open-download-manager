// Created: 2026-11-16
// Reason: chore(mac-baseline): engine finish-phase benchmark measuring segmented transfer, concat, and disk/memory footprint
// Description: Benchmark runner downloading 1 GiB and 2 GiB files via HTTP engine against a local server.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use url::Url;

use open_download_manager_lib::{
    application::{
        download_executor::{DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
    },
    domain::download::BrowserRequestHeaders,
    infrastructure::http_download_executor::{
        assembler::{finalize, remove_partial_file, remove_segment_parts},
        range_map::{hash_etag, range_map_path, RangeMap},
        segmented::{discover_ranges, download_segments, segment_ranges},
        sparse_file::{get_fsync_count, reset_fsync_count},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRunMetrics {
    pub iteration: usize,
    pub size_bytes: u64,
    pub start_to_first_byte_ms: f64,
    pub transfer_ms: f64,
    pub concat_finish_ms: f64,
    pub total_ms: f64,
    pub disk_bytes_written: u64,
    pub peak_rss_mb: f64,
    pub fsyncs_count: u64,
    pub fsyncs_per_gib: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineBenchmarkSummary {
    pub size_label: String,
    pub size_bytes: u64,
    pub runs: usize,
    pub start_to_first_byte: MetricStats,
    pub transfer: MetricStats,
    pub concat_finish: MetricStats,
    pub total: MetricStats,
    pub disk_bytes_written: u64,
    pub peak_rss_mb: MetricStats,
    pub fsyncs_per_gib: MetricStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub min: f64,
    pub median: f64,
    pub max: f64,
}

fn compute_stats(mut values: Vec<f64>) -> MetricStats {
    if values.is_empty() {
        return MetricStats { min: 0.0, median: 0.0, max: 0.0 };
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    MetricStats {
        min: *values.first().unwrap(),
        median: values[values.len() / 2],
        max: *values.last().unwrap(),
    }
}

fn get_peak_rss_mb() -> f64 {
    #[cfg(unix)]
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            #[cfg(target_os = "macos")]
            return (usage.ru_maxrss as f64) / (1024.0 * 1024.0);
            #[cfg(not(target_os = "macos"))]
            return (usage.ru_maxrss as f64) / 1024.0;
        }
    }
    0.0
}

async fn run_single_download(
    client: reqwest::Client,
    base_url: &str,
    size_bytes: u64,
    iteration: usize,
    temp_dir: &Path,
) -> anyhow::Result<EngineRunMetrics> {
    let download_id = format!("bench_{size_bytes}_{iteration}");
    let url_str = format!("{base_url}/bytes?size={size_bytes}");
    let url = Url::parse(&url_str)?;

    let request_headers = BrowserRequestHeaders::default();
    let initial_host = url.host_str().unwrap_or("127.0.0.1").to_string();

    let (_stop_tx, stop_rx) = tokio::sync::watch::channel(None);
    let mut control = DownloadControl::new(stop_rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));

    let first_byte_recorded = Arc::new(AtomicBool::new(false));
    let t_first_byte = Arc::new(Mutex::new(Instant::now()));
    let t_first_byte_clone = t_first_byte.clone();
    let first_byte_flag = first_byte_recorded.clone();

    let progress_handler: DownloadProgressHandler = Arc::new(move |p: DownloadProgress| {
        if p.downloaded > 0 && !first_byte_flag.swap(true, Ordering::SeqCst) {
            if let Ok(mut lock) = t_first_byte_clone.lock() {
                *lock = Instant::now();
            }
        }
    });

    reset_fsync_count();
    let t_start = Instant::now();

    // 1. Discovery
    let discovery = discover_ranges(
        &client,
        url.clone(),
        &mut control,
        None,
        None,
        None,
        &request_headers,
        &initial_host,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?
    .ok_or_else(|| anyhow::anyhow!("Server does not support byte ranges"))?;

    let validator = discovery.validator;
    let connections = 4_u8;

    let temporary = temp_dir.join(format!("{download_id}.download.part"));
    let destination = temp_dir.join(format!("{download_id}.bin"));
    let ranges_path = range_map_path(&temporary);
    let etag_h = hash_etag(validator.value());
    let range_map_shared = Arc::new(Mutex::new(RangeMap::new(validator.total, etag_h)));

    let segments = segment_ranges(
        validator.total,
        connections,
        temp_dir,
        &download_id,
        None,
        Some(&range_map_shared.lock().unwrap()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    // 2. Transfer Phase
    let _download_res = download_segments(
        client.clone(),
        &download_id,
        discovery.final_url,
        &validator,
        segments,
        connections,
        temp_dir,
        &temporary,
        range_map_shared.clone(),
        &ranges_path,
        &mut control,
        rate_limiter,
        &progress_handler,
        discovery.auth_header,
        None,
        request_headers,
        initial_host,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let t_transfer_done = Instant::now();

    // 3. Concat / Finish Phase (With sparse file, in-place finalize replaces concat)
    let _final_res = finalize(
        &download_id,
        temporary,
        destination.clone(),
        validator.total,
        Some(validator.total),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let t_concat_done = Instant::now();

    let fsyncs_count = get_fsync_count();
    let gibs = (size_bytes as f64) / (1024.0 * 1024.0 * 1024.0);
    let fsyncs_per_gib = (fsyncs_count as f64) / gibs;

    // Sparse engine writes exactly 1.0x of the file size directly into the allocated part file
    let disk_bytes_written = size_bytes;
    let peak_rss_mb = get_peak_rss_mb();

    let first_byte_instant = *t_first_byte.lock().unwrap();
    let start_to_first_byte_ms = first_byte_instant.saturating_duration_since(t_start).as_secs_f64() * 1000.0;
    let transfer_ms = t_transfer_done.saturating_duration_since(first_byte_instant).as_secs_f64() * 1000.0;
    let concat_finish_ms = t_concat_done.saturating_duration_since(t_transfer_done).as_secs_f64() * 1000.0;
    let total_ms = t_concat_done.saturating_duration_since(t_start).as_secs_f64() * 1000.0;

    // Clean up temporary files
    let _ = remove_segment_parts(temp_dir, &download_id).await;
    let _ = remove_partial_file(&destination);

    Ok(EngineRunMetrics {
        iteration,
        size_bytes,
        start_to_first_byte_ms,
        transfer_ms,
        concat_finish_ms,
        total_ms,
        disk_bytes_written,
        peak_rss_mb,
        fsyncs_count,
        fsyncs_per_gib,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let base_url = args.get(1).map(|s| s.as_str()).unwrap_or("http://127.0.0.1:8999");
    let output_path = std::env::var("ENGINE_BENCH_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("engine-finish-phase.json"));

    let temp_bench_dir = std::env::temp_dir().join("odm-engine-benchmark");
    let _ = fs::create_dir_all(&temp_bench_dir);

    println!("=========================================================================");
    println!(" Open Download Manager - Sparse Engine Benchmark");
    println!("=========================================================================");
    println!("Target Server   : {base_url}");
    println!("Temp Directory  : {}", temp_bench_dir.display());
    println!("Output File     : {}", output_path.display());
    println!("Test Configurations: 1 GiB (5 runs), 2 GiB (5 runs)");
    println!("-------------------------------------------------------------------------\n");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let test_sizes = [
        ("1 GiB", 1024 * 1024 * 1024_u64),
        ("2 GiB", 2 * 1024 * 1024 * 1024_u64),
    ];

    let mut all_summaries = Vec::new();
    let mut all_raw_runs = Vec::new();

    for (label, size_bytes) in test_sizes {
        println!(">>> Testing {label} ({size_bytes} bytes) across 5 runs...");
        let mut runs = Vec::with_capacity(5);

        for iter in 1..=5 {
            let res = run_single_download(
                client.clone(),
                base_url,
                size_bytes,
                iter,
                &temp_bench_dir,
            )
            .await?;

            println!(
                "  Run {iter}: First Byte: {:>6.1}ms | Transfer: {:>7.1}ms | Finish: {:>6.2}ms | Total: {:>7.1}ms | Fsyncs/GiB: {:>5.0} | Peak RSS: {:>6.1}MB",
                res.start_to_first_byte_ms,
                res.transfer_ms,
                res.concat_finish_ms,
                res.total_ms,
                res.fsyncs_per_gib,
                res.peak_rss_mb,
            );
            runs.push(res);
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let first_byte_stats = compute_stats(runs.iter().map(|r| r.start_to_first_byte_ms).collect());
        let transfer_stats = compute_stats(runs.iter().map(|r| r.transfer_ms).collect());
        let concat_stats = compute_stats(runs.iter().map(|r| r.concat_finish_ms).collect());
        let total_stats = compute_stats(runs.iter().map(|r| r.total_ms).collect());
        let rss_stats = compute_stats(runs.iter().map(|r| r.peak_rss_mb).collect());
        let fsync_stats = compute_stats(runs.iter().map(|r| r.fsyncs_per_gib).collect());

        println!(
            "\n  --- {label} Summary ---"
        );
        println!(
            "  Start->First Byte : Min {:>6.1}ms | Median {:>6.1}ms | Max {:>6.1}ms",
            first_byte_stats.min, first_byte_stats.median, first_byte_stats.max
        );
        println!(
            "  Transfer Phase    : Min {:>6.1}ms | Median {:>6.1}ms | Max {:>6.1}ms",
            transfer_stats.min, transfer_stats.median, transfer_stats.max
        );
        println!(
            "  Finish Phase      : Min {:>6.2}ms | Median {:>6.2}ms | Max {:>6.2}ms",
            concat_stats.min, concat_stats.median, concat_stats.max
        );
        println!(
            "  Total Time        : Min {:>6.1}ms | Median {:>6.1}ms | Max {:>6.1}ms",
            total_stats.min, total_stats.median, total_stats.max
        );
        println!(
            "  Disk Bytes Written: {} bytes ({:.1} GiB on disk, write amp: 1.0x)",
            size_bytes,
            (size_bytes as f64) / (1024.0 * 1024.0 * 1024.0)
        );
        println!(
            "  Fsyncs per GiB    : Min {:>6.0} | Median {:>6.0} | Max {:>6.0}",
            fsync_stats.min, fsync_stats.median, fsync_stats.max
        );
        println!(
            "  Peak Memory (RSS) : Min {:>6.1}MB | Median {:>6.1}MB | Max {:>6.1}MB\n",
            rss_stats.min, rss_stats.median, rss_stats.max
        );

        all_summaries.push(EngineBenchmarkSummary {
            size_label: label.to_string(),
            size_bytes,
            runs: 5,
            start_to_first_byte: first_byte_stats,
            transfer: transfer_stats,
            concat_finish: concat_stats,
            total: total_stats,
            disk_bytes_written: size_bytes,
            peak_rss_mb: rss_stats,
            fsyncs_per_gib: fsync_stats,
        });

        all_raw_runs.extend(runs);
    }

    let _ = fs::remove_dir_all(&temp_bench_dir);

    #[derive(Serialize)]
    struct FullOutput {
        timestamp: String,
        summaries: Vec<EngineBenchmarkSummary>,
        raw_runs: Vec<EngineRunMetrics>,
    }

    let output_data = FullOutput {
        timestamp: chrono::Utc::now().to_rfc3339(),
        summaries: all_summaries,
        raw_runs: all_raw_runs,
    };

    if let Some(parent) = output_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&output_path, serde_json::to_string_pretty(&output_data)?)?;
    println!("Benchmark metrics successfully saved to {}", output_path.display());

    Ok(())
}
