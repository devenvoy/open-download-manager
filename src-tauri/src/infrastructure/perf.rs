// Created: 2026-11-16
// Reason: chore(instrumentation): add tracing timers, rotating perf logs, and main thread watchdog (PR-A)
// Description: Tracing timers for IPC commands, snapshot serialization, startup timeline marks, and main thread latency.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock, RwLock,
    },
    time::{Duration, Instant},
};

const MAX_LOG_FILE_SIZE: u64 = 5 * 1024 * 1024; // 5 MB
const COMMAND_SLOW_THRESHOLD_MS: u64 = 100;
const MAIN_THREAD_STALL_THRESHOLD_MS: u64 = 250;

// Global process start time
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

pub fn init_process_start() {
    PROCESS_START.get_or_init(Instant::now);
}

pub fn get_process_start() -> Instant {
    *PROCESS_START.get_or_init(Instant::now)
}

// In-flight command registry
static NEXT_COMMAND_ID: AtomicU64 = AtomicU64::new(1);
static IN_FLIGHT_COMMANDS: OnceLock<RwLock<HashMap<u64, (String, Instant)>>> = OnceLock::new();

fn in_flight_map() -> &'static RwLock<HashMap<u64, (String, Instant)>> {
    IN_FLIGHT_COMMANDS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn record_command_start(id: u64, name: &str, start: Instant) {
    if let Ok(mut map) = in_flight_map().write() {
        map.insert(id, (name.to_string(), start));
    }
}

pub fn record_command_end(id: u64, name: &str, elapsed: Duration) {
    if let Ok(mut map) = in_flight_map().write() {
        map.remove(&id);
    }
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    if elapsed.as_millis() as u64 >= COMMAND_SLOW_THRESHOLD_MS {
        log_warn(
            "IPC_SLOW",
            &format!("{name} took {elapsed_ms:.1}ms (exceeds {COMMAND_SLOW_THRESHOLD_MS}ms threshold)"),
        );
    } else if is_verbose() {
        log_perf("IPC", &format!("{name} completed in {elapsed_ms:.2}ms"));
    }
}

pub fn track_command_start(name: &str) -> (u64, Instant) {
    let id = NEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed);
    let start = Instant::now();
    record_command_start(id, name, start);
    (id, start)
}

pub fn track_command_finish(id: u64, name: &str, elapsed: Duration) {
    record_command_end(id, name, elapsed);
}

pub fn get_in_flight_commands() -> Vec<String> {
    let now = Instant::now();
    if let Ok(map) = in_flight_map().read() {
        map.values()
            .map(|(name, start)| {
                let duration_ms = (now - *start).as_millis();
                format!("{name} ({duration_ms}ms)")
            })
            .collect()
    } else {
        Vec::new()
    }
}

pub struct CommandTimer {
    id: u64,
    name: String,
    start: Instant,
}

impl Drop for CommandTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        record_command_end(self.id, &self.name, elapsed);
    }
}

pub fn track_command(name: &str) -> CommandTimer {
    let id = NEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed);
    let start = Instant::now();
    record_command_start(id, name, start);
    CommandTimer {
        id,
        name: name.to_string(),
        start,
    }
}

// Rotating Perf Logger
pub struct PerfLogger {
    log_dir: PathBuf,
    log_file: Mutex<Option<(File, u64)>>,
    verbose: bool,
}

static LOGGER: OnceLock<PerfLogger> = OnceLock::new();

pub fn init_logger(data_dir: &std::path::Path) {
    let log_dir = data_dir.join("logs");
    let verbose = std::env::var("ODM_PERF")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let logger = PerfLogger::new(log_dir, verbose);
    let _ = LOGGER.set(logger);
    log_info("LOGGER", "Performance tracing logger initialized");
}

pub fn is_verbose() -> bool {
    LOGGER.get().map(|l| l.verbose).unwrap_or(false)
}

impl PerfLogger {
    pub fn new(log_dir: PathBuf, verbose: bool) -> Self {
        let _ = fs::create_dir_all(&log_dir);
        Self {
            log_dir,
            log_file: Mutex::new(None),
            verbose,
        }
    }

    pub fn write_formatted(&self, formatted: &str, max_file_size: u64) {
        if let Ok(mut lock) = self.log_file.lock() {
            if lock.is_none() {
                let primary_path = self.log_dir.join("odm-perf.log");
                let current_size = fs::metadata(&primary_path).map(|m| m.len()).unwrap_or(0);
                if let Ok(file) = OpenOptions::new().create(true).append(true).open(&primary_path) {
                    *lock = Some((file, current_size));
                }
            }

            if let Some((ref mut file, ref mut size)) = *lock {
                let bytes = formatted.as_bytes();
                if file.write_all(bytes).is_ok() {
                    *size += bytes.len() as u64;
                    let _ = file.flush();
                }

                if *size >= max_file_size {
                    // Rotate logs: odm-perf.1.log -> odm-perf.2.log; odm-perf.log -> odm-perf.1.log
                    let p2 = self.log_dir.join("odm-perf.2.log");
                    let p1 = self.log_dir.join("odm-perf.1.log");
                    let p0 = self.log_dir.join("odm-perf.log");

                    drop(lock); // Release lock before filesystem mutations
                    let _ = fs::remove_file(&p2);
                    let _ = fs::rename(&p1, &p2);
                    let _ = fs::rename(&p0, &p1);

                    // Re-acquire lock to open new primary log file
                    if let Ok(mut lock_after) = self.log_file.lock() {
                        if let Ok(new_file) = OpenOptions::new().create(true).append(true).open(&p0) {
                            *lock_after = Some((new_file, 0));
                        } else {
                            *lock_after = None;
                        }
                    }
                }
            }
        }
    }
}

fn write_to_log(level: &str, target: &str, message: &str) {
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let formatted = format!("[{timestamp}] [{level}] [{target}] {message}\n");

    let verbose = is_verbose();
    if verbose || level == "WARN" || level == "ERROR" {
        eprint!("{formatted}");
    }

    if let Some(logger) = LOGGER.get() {
        logger.write_formatted(&formatted, MAX_LOG_FILE_SIZE);
    }
}

pub fn log_info(target: &str, message: &str) {
    write_to_log("INFO", target, message);
}

pub fn log_warn(target: &str, message: &str) {
    write_to_log("WARN", target, message);
}

pub fn log_perf(target: &str, message: &str) {
    write_to_log("PERF", target, message);
}

pub fn log_debug(target: &str, message: &str) {
    if is_verbose() {
        write_to_log("DEBUG", target, message);
    }
}

// Startup Timeline Tracer
#[derive(Debug, Clone)]
pub struct StartupTimeline {
    pub process_start: Instant,
    pub setup_start: Option<Instant>,
    pub engine_start: Option<Instant>,
    pub setup_done: Option<Instant>,
    pub main_window_created: Option<Instant>,
    pub main_window_shown: Option<Instant>,
    pub ui_ready: Option<Instant>,
}

impl Default for StartupTimeline {
    fn default() -> Self {
        Self {
            process_start: get_process_start(),
            setup_start: None,
            engine_start: None,
            setup_done: None,
            main_window_created: None,
            main_window_shown: None,
            ui_ready: None,
        }
    }
}

impl StartupTimeline {
    pub fn format_timeline(&self) -> String {
        let mut marks: Vec<(Duration, &'static str)> = Vec::new();
        marks.push((Duration::ZERO, "Process Start"));
        if let Some(t) = self.setup_start {
            marks.push((t.saturating_duration_since(self.process_start), "Setup Start"));
        }
        if let Some(t) = self.engine_start {
            marks.push((t.saturating_duration_since(self.process_start), "Engine Start"));
        }
        if let Some(t) = self.main_window_created {
            marks.push((t.saturating_duration_since(self.process_start), "Window Created"));
        }
        if let Some(t) = self.setup_done {
            marks.push((t.saturating_duration_since(self.process_start), "Setup Done"));
        }
        if let Some(t) = self.main_window_shown {
            marks.push((t.saturating_duration_since(self.process_start), "Window Shown"));
        }
        if let Some(t) = self.ui_ready {
            marks.push((t.saturating_duration_since(self.process_start), "UI Ready"));
        }

        marks.sort_by_key(|(dur, _)| *dur);

        let parts: Vec<String> = marks
            .iter()
            .map(|(dur, name)| {
                let ms = dur.as_secs_f64() * 1000.0;
                if *name == "Process Start" {
                    format!("{name}: 0.0ms")
                } else {
                    format!("{name}: +{ms:.1}ms")
                }
            })
            .collect();

        let total_ms = marks.last().map(|(d, _)| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        format!("Startup Timeline (Total: {total_ms:.1}ms) | {}", parts.join(" -> "))
    }
}

static TIMELINE: OnceLock<Mutex<StartupTimeline>> = OnceLock::new();

fn timeline() -> &'static Mutex<StartupTimeline> {
    TIMELINE.get_or_init(|| Mutex::new(StartupTimeline::default()))
}

pub fn mark_setup_start() {
    if let Ok(mut t) = timeline().lock() {
        t.setup_start = Some(Instant::now());
        let elapsed = t.process_start.elapsed().as_secs_f64() * 1000.0;
        log_info("STARTUP", &format!("Setup started at +{elapsed:.1}ms from process start"));
    }
}

pub fn mark_engine_start() {
    if let Ok(mut t) = timeline().lock() {
        t.engine_start = Some(Instant::now());
        let elapsed = t.process_start.elapsed().as_secs_f64() * 1000.0;
        log_info("STARTUP", &format!("Engines initializing at +{elapsed:.1}ms from process start"));
    }
}

pub fn mark_setup_done() {
    if let Ok(mut t) = timeline().lock() {
        t.setup_done = Some(Instant::now());
        let elapsed = t.process_start.elapsed().as_secs_f64() * 1000.0;
        log_info("STARTUP", &format!("Setup completed at +{elapsed:.1}ms from process start"));
    }
}

pub fn mark_main_window_created() {
    if let Ok(mut t) = timeline().lock() {
        t.main_window_created = Some(Instant::now());
        let elapsed = t.process_start.elapsed().as_secs_f64() * 1000.0;
        log_info("STARTUP", &format!("Main window created at +{elapsed:.1}ms from process start"));
    }
}

pub fn mark_main_window_shown() {
    if let Ok(mut t) = timeline().lock() {
        t.main_window_shown = Some(Instant::now());
        let elapsed = t.process_start.elapsed().as_secs_f64() * 1000.0;
        log_info("STARTUP", &format!("Main window shown at +{elapsed:.1}ms from process start"));
        let timeline_str = t.format_timeline();
        log_perf("STARTUP_TIMELINE", &timeline_str);
    }
}

pub fn mark_ui_ready() {
    if let Ok(mut t) = timeline().lock() {
        if t.ui_ready.is_none() {
            t.ui_ready = Some(Instant::now());
            let elapsed = t.process_start.elapsed().as_secs_f64() * 1000.0;
            log_info("STARTUP", &format!("UI ready at +{elapsed:.1}ms from process start"));
            let timeline_str = t.format_timeline();
            log_perf("STARTUP_TIMELINE", &timeline_str);
        }
    }
}

// Auxiliary Window open->shown timing
pub fn record_window_shown(label: &str, elapsed: Duration) {
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    log_perf("WINDOW", &format!("Aux window '{label}' open->shown took {elapsed_ms:.1}ms"));
    if elapsed.as_millis() as u64 >= 250 {
        log_warn("WINDOW_SLOW", &format!("Aux window '{label}' open->shown took {elapsed_ms:.1}ms (>250ms threshold)"));
    }
}

// Main-thread latency ping watchdog
pub fn start_main_thread_watchdog(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        loop {
            interval.tick().await;
            let ping_start = Instant::now();
            let (tx, rx) = tokio::sync::oneshot::channel();

            let run_res = app.run_on_main_thread(move || {
                let latency = ping_start.elapsed();
                let _ = tx.send(latency);
            });

            if run_res.is_ok() {
                match tokio::time::timeout(Duration::from_millis(5000), rx).await {
                    Ok(Ok(latency)) => {
                        let latency_ms = latency.as_secs_f64() * 1000.0;
                        if latency.as_millis() as u64 >= MAIN_THREAD_STALL_THRESHOLD_MS {
                            let in_flight = get_in_flight_commands();
                            let in_flight_desc = if in_flight.is_empty() {
                                "none".to_string()
                            } else {
                                in_flight.join(", ")
                            };
                            log_warn(
                                "MAIN_THREAD_STALL",
                                &format!(
                                    "Main thread latency ping delayed by {latency_ms:.1}ms (threshold: {MAIN_THREAD_STALL_THRESHOLD_MS}ms). In-flight commands: [{in_flight_desc}]"
                                ),
                            );
                        } else if is_verbose() {
                            log_debug("MAIN_THREAD_PING", &format!("Latency: {latency_ms:.2}ms"));
                        }
                    }
                    _ => {
                        let in_flight = get_in_flight_commands();
                        let in_flight_desc = if in_flight.is_empty() {
                            "none".to_string()
                        } else {
                            in_flight.join(", ")
                        };
                        log_warn(
                            "MAIN_THREAD_FREEZE",
                            &format!(
                                "Main thread ping timed out after 5000ms! In-flight commands: [{in_flight_desc}]"
                            ),
                        );
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_timer_and_in_flight_tracking() {
        let cmd_name = "test_perf_command";
        let timer = track_command(cmd_name);
        let in_flight = get_in_flight_commands();
        assert!(
            in_flight.iter().any(|c| c.contains(cmd_name)),
            "In-flight commands must contain active command: {in_flight:?}"
        );

        drop(timer);
        let in_flight_after = get_in_flight_commands();
        assert!(
            !in_flight_after.iter().any(|c| c.contains(cmd_name)),
            "In-flight commands must not contain dropped command: {in_flight_after:?}"
        );
    }

    #[test]
    fn test_startup_timeline_recording() {
        mark_setup_start();
        mark_engine_start();
        mark_setup_done();
        mark_main_window_created();
        mark_main_window_shown();
        mark_ui_ready();

        let t = timeline().lock().unwrap();
        assert!(t.setup_start.is_some());
        assert!(t.engine_start.is_some());
        assert!(t.setup_done.is_some());
        assert!(t.main_window_created.is_some());
        assert!(t.main_window_shown.is_some());
        assert!(t.ui_ready.is_some());

        let formatted = t.format_timeline();
        assert!(formatted.contains("Startup Timeline"));
        assert!(formatted.contains("UI Ready:"));
    }

    #[test]
    fn test_perf_logger_rotation() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let log_dir = temp_dir.path().join("logs");
        let logger = PerfLogger::new(log_dir.clone(), true);

        // Each line is 60 bytes. With threshold 50 bytes, each write triggers rotation.
        let line = "12345678901234567890123456789012345678901234567890123456789\n";
        assert_eq!(line.len(), 60);

        // Write 1: creates odm-perf.log and rotates to odm-perf.1.log
        logger.write_formatted(line, 50);
        let p1 = log_dir.join("odm-perf.1.log");
        assert!(p1.exists(), "First rotated log odm-perf.1.log must exist after 1st rotation");

        // Write 2: creates odm-perf.log, rotates odm-perf.1.log -> odm-perf.2.log
        logger.write_formatted(line, 50);
        let p2 = log_dir.join("odm-perf.2.log");
        assert!(p2.exists(), "Second rotated log odm-perf.2.log must exist after 2nd rotation");

        let p0 = log_dir.join("odm-perf.log");
        assert!(p0.exists(), "Primary log file must exist");
    }
}
