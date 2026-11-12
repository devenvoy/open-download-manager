// Created: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Last updated: 2026-11-08
// Reason: feat(browser): add configurable context menu actions & file capture rules (#29)
// Description: Browser native messaging host binary bridging Chrome/Firefox extensions to the desktop application.

use std::{
    io::{self, Read, Write},
    path::PathBuf,
};
use serde::{Deserialize, Serialize};

const MAX_MESSAGE: usize = 1024 * 1024;

#[derive(Deserialize, Serialize)]
struct BridgeConfig {
    port: u16,
    token: String,
}

#[derive(Debug)]
struct HostError {
    message: String,
    /// False when the desktop app is not running, so the extension can back
    /// off instead of retrying every minute.
    app_running: bool,
}

impl HostError {
    fn new(message: impl Into<String>, app_running: bool) -> Self {
        Self {
            message: message.into(),
            app_running,
        }
    }
}

impl From<String> for HostError {
    fn from(message: String) -> Self {
        Self::new(message, true)
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        let _ = write_message(&serde_json::json!({
            "error": error.message,
            "appRunning": error.app_running,
        }));
    }
}

async fn run() -> Result<(), HostError> {
    let msg = read_message()?;
    let bridge_config = ensure_bridge_config(false).await?;

    let action = msg
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostError::new("Native message must specify an action.", true))?;

    let endpoint = match action {
        "formats" => "/formats",
        "download" => "/downloads",
        "refresh" => "/downloads/refresh",
        "policy" | "capturePolicy" => "/policy",
        "torrent" => "/torrents",
        "heartbeat" => "/heartbeat",
        _ => {
            return Err(HostError::new("Unsupported native action.", true));
        }
    };

    // A heartbeat only reports that the extension is alive. It must never
    // start the desktop app: that would relaunch the app every minute.
    let may_launch_desktop = action != "heartbeat";

    let url = format!("http://127.0.0.1:{}{endpoint}", bridge_config.port);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| HostError::new(e.to_string(), true))?;

    let payload = serde_json::to_vec(&msg).map_err(|e| HostError::new(e.to_string(), true))?;
    let resp = send_with_retry(&client, &url, &bridge_config, &payload, may_launch_desktop).await?;

    let status = resp.status();
    let resp_bytes = resp
        .bytes()
        .await
        .map_err(|e| HostError::new(format!("Could not read response from desktop app: {e}"), true))?;

    let body: serde_json::Value = serde_json::from_slice(&resp_bytes)
        .map_err(|e| HostError::new(format!("Invalid response from desktop app: {e}"), true))?;

    if !status.is_success() {
        let err_msg = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Desktop application error.");
        return Err(HostError::new(err_msg, true));
    }

    write_message(&body).map_err(|e| HostError::new(e.to_string(), true))?;
    Ok(())
}

async fn send_with_retry(
    client: &reqwest::Client,
    url: &str,
    config: &BridgeConfig,
    payload: &[u8],
    may_launch_desktop: bool,
) -> Result<reqwest::Response, HostError> {
    match client
        .post(url)
        .header("Authorization", format!("Bearer {}", config.token))
        .header("Content-Type", "application/json")
        .header("Host", format!("127.0.0.1:{}", config.port))
        .body(payload.to_vec())
        .send()
        .await
    {
        Ok(resp) => Ok(resp),
        Err(first_err) => {
            if may_launch_desktop {
                try_launch_desktop();
            }
            for _ in 0..20 {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                if let Ok(new_cfg) = load_bridge_config() {
                    let new_url = format!("http://127.0.0.1:{}{}", new_cfg.port, url.split_once("127.0.0.1:").map(|(_, rest)| rest.split_once('/').map(|(_, p)| format!("/{p}")).unwrap_or_default()).unwrap_or_default());
                    if let Ok(resp) = client
                        .post(&new_url)
                        .header("Authorization", format!("Bearer {}", new_cfg.token))
                        .header("Content-Type", "application/json")
                        .header("Host", format!("127.0.0.1:{}", new_cfg.port))
                        .body(payload.to_vec())
                        .send()
                        .await
                    {
                        return Ok(resp);
                    }
                }
            }
            Err(HostError::new(
                format!("Could not connect to desktop app: {first_err}"),
                false,
            ))
        }
    }
}

fn find_desktop_executable() -> Option<PathBuf> {
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            #[cfg(windows)]
            let candidate = dir.join("open-download-manager.exe");
            #[cfg(not(windows))]
            let candidate = dir.join("open-download-manager");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn try_launch_desktop() {
    // Prevent multiple concurrent desktop launches when several native messages
    // arrive at once (e.g., extension content scripts on many tabs). Only one
    // launch is allowed per 10-second window; tauri-plugin-single-instance
    // ensures subsequent instances focus the existing window instead of opening
    // new ones.
    let lock_path = std::env::temp_dir().join("odm_launch.lock");
    if let Ok(metadata) = std::fs::metadata(&lock_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed.as_secs() < 10 {
                    return;
                }
            }
        }
    }
    let _ = std::fs::File::create(&lock_path);

    if let Some(exe) = find_desktop_executable() {
        let _ = std::process::Command::new(exe)
            .arg("--browser-download")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

async fn ensure_bridge_config(may_launch_desktop: bool) -> Result<BridgeConfig, HostError> {
    if let Ok(cfg) = load_bridge_config() {
        return Ok(cfg);
    }
    if may_launch_desktop {
        try_launch_desktop();
    }
    let attempts = if may_launch_desktop { 30 } else { 1 };
    for _ in 0..attempts {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if let Ok(cfg) = load_bridge_config() {
            return Ok(cfg);
        }
    }
    Err(HostError::new(
        "Open Download Manager is not running. Launch the desktop app first.",
        false,
    ))
}

fn load_bridge_config() -> Result<BridgeConfig, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Could not find user home directory.".to_owned())?;
    let path = home.join(".open-download-manager").join("bridge.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|_| "Open Download Manager is not running. Launch the desktop app first.".to_owned())?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Invalid bridge.json: {e}"))
}

fn read_message() -> Result<serde_json::Value, String> {
    let mut stdin = io::stdin().lock();
    let mut length_bytes = [0u8; 4];
    stdin
        .read_exact(&mut length_bytes)
        .map_err(|e| format!("Could not read message length from stdin: {e}"))?;

    let length = u32::from_le_bytes(length_bytes) as usize;
    if length == 0 || length > MAX_MESSAGE {
        return Err("Native message size is invalid.".to_owned());
    }

    let mut body = vec![0u8; length];
    stdin
        .read_exact(&mut body)
        .map_err(|e| format!("Could not read message body: {e}"))?;

    serde_json::from_slice(&body)
        .map_err(|e| format!("Native message must be valid JSON: {e}"))
}

fn write_message(value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_MESSAGE {
        return Err("Response is too large.".to_owned());
    }
    let length = (bytes.len() as u32).to_le_bytes();
    let mut stdout = io::stdout().lock();
    stdout.write_all(&length).map_err(|e| e.to_string())?;
    stdout.write_all(&bytes).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())?;
    Ok(())
}
