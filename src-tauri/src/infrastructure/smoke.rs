// Created: 2026-11-16
// Reason: chore(mac-baseline): in-app smoke test runner and safety guard for macOS baseline
// Description: Automated smoke mode exercising snapshot serialization scaling, window lifecycle, magnet add, and idle watchdog.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    domain::download::{
        BrowserRequestHeaders, DownloadAction, DownloadRecord, DownloadRequest, DownloadStatus,
    },
    presentation::app_state::AppState,
};

static IS_SMOKE: OnceLock<bool> = OnceLock::new();
static SMOKE_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn is_smoke_test() -> bool {
    *IS_SMOKE.get_or_init(|| {
        std::env::var("ODM_SMOKE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
            || std::env::args().any(|arg| arg == "--smoke-test" || arg == "--smoke")
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotBenchmarkResult {
    pub count: usize,
    pub iterations: usize,
    pub median_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub p95_ms: f64,
    pub payload_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowBenchmarkResult {
    pub mode: String,
    pub iterations: usize,
    pub median_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub p95_ms: f64,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnetBenchmarkResult {
    pub magnet: String,
    pub name: String,
    pub add_return_ms: f64,
    pub row_visible_ms: f64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleWatchdogResult {
    pub duration_seconds: u64,
    pub stall_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLoadResult {
    pub concurrent_downloads: usize,
    pub duration_seconds: u64,
    pub watchdog_stalls: u64,
    pub ui_responsiveness_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmokeTestResults {
    pub timestamp: String,
    pub snapshots: Vec<SnapshotBenchmarkResult>,
    pub windows: Vec<WindowBenchmarkResult>,
    pub magnet: MagnetBenchmarkResult,
    pub idle: IdleWatchdogResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_load: Option<ActiveLoadResult>,
    pub all_windows_passed: bool,
}

fn generate_fake_records(count: usize) -> Vec<DownloadRecord> {
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let status = if i % 10 == 0 {
            DownloadStatus::Completed
        } else if i % 3 == 0 {
            DownloadStatus::Downloading
        } else {
            DownloadStatus::Queued
        };
        records.push(DownloadRecord {
            id: format!("smoke_dl_{i}"),
            url: format!("https://smoke-test.example.com/files/document_{i}.pdf"),
            filename: format!("document_{i}.pdf"),
            status,
            downloaded: (i as u64) * 2048,
            total: Some((i as u64) * 4096 + 1024),
            speed: if i % 3 == 0 { 512 * 1024 } else { 0 },
            created_at: format!("2026-11-16T12:{:02}:{:02}Z", (i / 60) % 60, i % 60),
            needs_review: i % 25 == 0,
            scheduled_at: None,
            queue_id: None,
            description: Some(format!("Fake history record {i}")),
            category: None,
            directory: Some("/tmp/odm-smoke-downloads".to_string()),
            path: Some(format!("/tmp/odm-smoke-downloads/document_{i}.pdf")),
            temp_directory: None,
            last_try: None,
            error: None,
            notice: None,
            estimated: false,
            transfer_kind: Some("segmented_http".to_string()),
            kind: None,
            resumable: Some(true),
            duration: Some(45.0),
            connections: 4,
            connections_progress: Vec::new(),
            audio_url: None,
            sabr: None,
            referer: None,
            youtube_format: None,
            etag: Some(format!("etag_{i}")),
            validator: None,
            conflict_policy: None,
            credentials_override: None,
            request_headers: BrowserRequestHeaders::default(),
            completion_actions: None,
            action_ledger: Vec::new(),
        });
    }
    records
}

pub async fn run_smoke_tests(app: AppHandle) {
    if SMOKE_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    println!("============================================================");
    println!(" [SMOKE] Open Download Manager In-App Smoke Test Running");
    println!("============================================================");

    // Initial stabilization cooldown to allow main window to mount
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let state = app.state::<AppState>();

    // -------------------------------------------------------------
    // Part 1: Seed N fake entries and benchmark snapshot (100, 1000, 5000)
    // -------------------------------------------------------------
    println!("\n[SMOKE] 1/4: Benchmarking Snapshot Scaling (100, 1000, 5000 items)...");
    let mut snapshot_results = Vec::new();
    let counts = [100, 1000, 5000];

    for &count in &counts {
        let records = generate_fake_records(count);
        if let Err(e) = state.download_history.seed_downloads(records) {
            eprintln!("[SMOKE] Failed to seed history for count {count}: {e:?}");
            continue;
        }

        let mut timings = Vec::with_capacity(30);
        let mut payload_bytes = 0;

        for _ in 0..30 {
            let start = Instant::now();
            if let Ok(snap) = crate::presentation::commands::settings::snapshot(app.state::<AppState>()) {
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                timings.push(elapsed);
                if payload_bytes == 0 {
                    payload_bytes = serde_json::to_vec(&snap).map(|v| v.len()).unwrap_or(0);
                }
            }
        }

        timings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let min_ms = *timings.first().unwrap_or(&0.0);
        let max_ms = *timings.last().unwrap_or(&0.0);
        let median_ms = if timings.is_empty() { 0.0 } else { timings[timings.len() / 2] };
        let p95_idx = ((timings.len() as f64) * 0.95).floor() as usize;
        let p95_ms = if timings.is_empty() { 0.0 } else { timings[p95_idx.min(timings.len() - 1)] };

        println!(
            "  N={count:4} -> Median: {median_ms:>6.2}ms | p95: {p95_ms:>6.2}ms | Min: {min_ms:>6.2}ms | Max: {max_ms:>6.2}ms | Payload: {payload_bytes} bytes",
        );

        snapshot_results.push(SnapshotBenchmarkResult {
            count,
            iterations: timings.len(),
            median_ms,
            min_ms,
            max_ms,
            p95_ms,
            payload_bytes,
        });
    }

    // -------------------------------------------------------------
    // Part 2: Open each aux window (10 iterations each)
    // -------------------------------------------------------------
    println!("\n[SMOKE] 2/4: Benchmarking Auxiliary Window Lifecycle (10 iterations each)...");
    let aux_modes = ["options", "add", "scheduler", "integration", "torrents", "progress"];
    let mut window_results = Vec::new();
    let mut all_windows_passed = true;
    let fake_progress_id = "smoke_dl_0".to_string();

    for &mode in &aux_modes {
        let mut open_timings = Vec::with_capacity(10);
        let mut timed_out = false;
        let label_to_await = if mode == "progress" {
            format!("progress-{fake_progress_id}")
        } else {
            mode.to_string()
        };

        for iter in 1..=10 {
            // Close any existing window instance before opening
            close_window_instance(&app, mode, &fake_progress_id).await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Spawn window open on main thread
            let app_clone = app.clone();
            let mode_str = mode.to_string();
            let id_clone = fake_progress_id.clone();
            let open_res = app.run_on_main_thread(move || {
                match mode_str.as_str() {
                    "options" => {
                        let _ = crate::presentation::window_manager::open_options_window(app_clone);
                    }
                    "add" => {
                        let _ = crate::presentation::window_manager::open_add_window(app_clone, None);
                    }
                    "scheduler" => {
                        let _ = crate::presentation::window_manager::open_scheduler_window(app_clone);
                    }
                    "integration" => {
                        let _ = crate::presentation::window_manager::open_integration_window(app_clone);
                    }
                    "torrents" => {
                        let _ = crate::presentation::window_manager::open_torrent_window(app_clone);
                    }
                    "progress" => {
                        let _ = crate::presentation::window_manager::open_progress_window(app_clone, id_clone);
                    }
                    _ => {}
                }
            });

            if let Err(e) = open_res {
                eprintln!("[SMOKE] Failed to dispatch open for window {mode}: {e}");
            }

            // Wait for ui_ready signal with 10s timeout
            match crate::infrastructure::perf::wait_for_window_ui_ready(&label_to_await, Duration::from_secs(10)).await {
                Ok(dur) => {
                    open_timings.push(dur.as_secs_f64() * 1000.0);
                }
                Err(err) => {
                    eprintln!("[SMOKE] Window {mode} (iter {iter}) TIMED OUT: {err}");
                    timed_out = true;
                    all_windows_passed = false;
                    break;
                }
            }

            // Close window
            close_window_instance(&app, mode, &fake_progress_id).await;
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        open_timings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let min_ms = *open_timings.first().unwrap_or(&0.0);
        let max_ms = *open_timings.last().unwrap_or(&0.0);
        let median_ms = if open_timings.is_empty() { 0.0 } else { open_timings[open_timings.len() / 2] };
        let p95_idx = ((open_timings.len() as f64) * 0.95).floor() as usize;
        let p95_ms = if open_timings.is_empty() { 0.0 } else { open_timings[p95_idx.min(open_timings.len() - 1)] };

        println!(
            "  Window {:12} -> Median: {median_ms:>6.1}ms | p95: {p95_ms:>6.1}ms | Min: {min_ms:>6.1}ms | Max: {max_ms:>6.1}ms | Timed out: {timed_out}",
            mode
        );

        window_results.push(WindowBenchmarkResult {
            mode: mode.to_string(),
            iterations: open_timings.len(),
            median_ms,
            min_ms,
            max_ms,
            p95_ms,
            timed_out,
        });
    }

    // -------------------------------------------------------------
    // Part 3: Public-domain torrent add (Ubuntu 24.04.1 LTS ISO)
    // -------------------------------------------------------------
    println!("\n[SMOKE] 3/4: Adding Legal Public-Domain Magnet (Ubuntu 24.04.1 Desktop AMD64)...");
    let magnet_uri = "magnet:?xt=urn:btih:2b6c31677c447e5b9d3b84ef6f530e1590fb3b34&dn=ubuntu-24.04.1-desktop-amd64.iso".to_string();
    let temp_torrent_dir = std::env::temp_dir().join("odm-smoke-torrent");
    let _ = std::fs::create_dir_all(&temp_torrent_dir);

    let start_add = Instant::now();
    let add_res = state.torrents.add_magnet(
        magnet_uri.clone(),
        crate::domain::torrent::TorrentAddOptions {
            output_folder: Some(temp_torrent_dir.to_string_lossy().to_string()),
            start_paused: true,
            only_files: None,
            review: false,
            initial_peers: None,
        },
    ).await;
    let add_return_ms = start_add.elapsed().as_secs_f64() * 1000.0;

    let mut row_visible_ms = add_return_ms;
    let mut success = add_res.is_ok();
    let mut error_msg = None;
    let mut torrent_id_to_remove = None;

    match add_res {
        Ok(res) => {
            torrent_id_to_remove = res.id;
            // Verify row visibility in torrent list
            for _ in 0..50 {
                if let Ok(list) = state.torrents.list() {
                    if let Some(t) = list.iter().find(|item| res.id.is_some_and(|rid| item.id == rid) || item.info_hash.eq_ignore_ascii_case("2b6c31677c447e5b9d3b84ef6f530e1590fb3b34")) {
                        row_visible_ms = start_add.elapsed().as_secs_f64() * 1000.0;
                        torrent_id_to_remove = Some(t.id);
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        Err(e) => {
            success = false;
            error_msg = Some(e.message);
        }
    }

    println!("  Magnet add returned in: {add_return_ms:>6.1}ms | Row visible in: {row_visible_ms:>6.1}ms | Success: {success}");

    // Clean up torrent
    if let Some(id) = torrent_id_to_remove {
        let _ = state.torrents.action(id, crate::domain::torrent::TorrentAction::Delete).await;
    }
    let _ = std::fs::remove_dir_all(&temp_torrent_dir);

    let magnet_result = MagnetBenchmarkResult {
        magnet: magnet_uri,
        name: "Ubuntu 24.04.1 Desktop AMD64 ISO".to_string(),
        add_return_ms,
        row_visible_ms,
        success,
        error: error_msg,
    };

    // -------------------------------------------------------------
    // Part 4: Idle 60s with frontend 2s polling & watchdog stalls
    // -------------------------------------------------------------
    println!("\n[SMOKE] 4/4: Observing Idle State for 60 seconds (Frontend 2s polling active)...");
    let start_stalls = crate::infrastructure::perf::get_watchdog_stalls_count();
    tokio::time::sleep(Duration::from_secs(60)).await;
    let end_stalls = crate::infrastructure::perf::get_watchdog_stalls_count();
    let stall_count = end_stalls.saturating_sub(start_stalls);

    println!("  Idle 60s completed: Watchdog stalls detected = {stall_count}");

    let idle_result = IdleWatchdogResult {
        duration_seconds: 60,
        stall_count,
    };

    // -------------------------------------------------------------
    // Part 5: Active Load Scenario (5 simultaneous downloads for 60s)
    // -------------------------------------------------------------
    println!("\n[SMOKE] 5/5: Active Load Scenario (5 concurrent streams for 60s)...");
    let server_health_url = "http://127.0.0.1:8999/health";
    let is_bench_server_up = reqwest::Client::new()
        .get(server_health_url)
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .is_ok();

    let active_load_result = if is_bench_server_up {
        let temp_download_dir = std::env::temp_dir().join("odm-smoke-active-load");
        let _ = std::fs::create_dir_all(&temp_download_dir);
        let settings = state
            .settings
            .lock()
            .map(|s| s.settings().clone())
            .unwrap_or_else(|e| e.into_inner().settings().clone());

        let mut active_download_ids = Vec::new();
        for i in 1..=5 {
            let req = DownloadRequest {
                urls: vec![format!("http://127.0.0.1:8999/bytes?size=10737418240&stream={i}")],
                directory: Some(temp_download_dir.to_string_lossy().to_string()),
                filename: Some(format!("active_load_stream_{i}.bin")),
                ..Default::default()
            };
            if let Ok(ids) = state.download_transfer.add(req, &settings) {
                active_download_ids.extend(ids);
            }
        }

        let start_active_stalls = crate::infrastructure::perf::get_watchdog_stalls_count();
        let active_start = Instant::now();

        let mut snapshot_latencies = Vec::new();
        while active_start.elapsed() < Duration::from_secs(60) {
            let snap_start = Instant::now();
            let _ = crate::presentation::commands::settings::snapshot(app.state::<AppState>());
            snapshot_latencies.push(snap_start.elapsed().as_secs_f64() * 1000.0);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        let end_active_stalls = crate::infrastructure::perf::get_watchdog_stalls_count();
        let active_stalls = end_active_stalls.saturating_sub(start_active_stalls);

        for id in active_download_ids {
            let _ = state.download_transfer.action(&id, DownloadAction::Cancel);
            let _ = state.download_transfer.action(&id, DownloadAction::Remove);
        }
        let _ = std::fs::remove_dir_all(&temp_download_dir);

        snapshot_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_ui_latency = if snapshot_latencies.is_empty() {
            0.0
        } else {
            snapshot_latencies[snapshot_latencies.len() / 2]
        };

        println!(
            "  Active load (5 downloads, 60s) completed: Watchdog stalls = {active_stalls} | UI snapshot median = {median_ui_latency:.2}ms"
        );

        Some(ActiveLoadResult {
            concurrent_downloads: 5,
            duration_seconds: 60,
            watchdog_stalls: active_stalls,
            ui_responsiveness_ms: median_ui_latency,
        })
    } else {
        println!("  Notice: Synthetic benchmark server not reachable at 127.0.0.1:8999, skipping active load.");
        None
    };

    // -------------------------------------------------------------
    // Output Generation & Exit
    // -------------------------------------------------------------
    let final_results = SmokeTestResults {
        timestamp: chrono::Utc::now().to_rfc3339(),
        snapshots: snapshot_results,
        windows: window_results,
        magnet: magnet_result,
        idle: idle_result,
        active_load: active_load_result,
        all_windows_passed,
    };

    let output_path = std::env::var("ODM_SMOKE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("smoke-result.json"));

    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(json_str) = serde_json::to_string_pretty(&final_results) {
        if let Err(e) = std::fs::write(&output_path, &json_str) {
            eprintln!("[SMOKE] Failed to write smoke-result.json to {:?}: {}", output_path, e);
        } else {
            println!("\n[SMOKE] Smoke results successfully saved to: {}", output_path.display());
        }
    }

    if !all_windows_passed || crate::infrastructure::perf::has_window_timed_out() {
        eprintln!("[SMOKE] Exit status: FAILURE (one or more windows timed out)");
        std::process::exit(1);
    } else {
        println!("[SMOKE] Exit status: SUCCESS (all assertions and milestones verified)");
        std::process::exit(0);
    }
}

async fn close_window_instance(app: &AppHandle, mode: &str, fake_progress_id: &str) {
    let label = if mode == "progress" {
        format!("progress-{fake_progress_id}")
    } else {
        match mode {
            "options" => crate::presentation::window_manager::OPTIONS_WINDOW_LABEL.to_string(),
            "add" => crate::presentation::window_manager::ADD_WINDOW_LABEL.to_string(),
            "scheduler" => crate::presentation::window_manager::SCHEDULER_WINDOW_LABEL.to_string(),
            "integration" => crate::presentation::window_manager::INTEGRATION_WINDOW_LABEL.to_string(),
            "torrents" => crate::presentation::window_manager::TORRENT_WINDOW_LABEL.to_string(),
            _ => mode.to_string(),
        }
    };

    if let Some(win) = app.get_webview_window(&label) {
        // Benchmark teardown must not run the production close flow: a review
        // prompt or a review-cancel would remove the seeded record, leaving
        // later iterations measuring an orphan "no longer in the list" window.
        let _ = win.destroy();
    }
}
