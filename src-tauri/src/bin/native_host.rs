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

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let _ = write_message(&serde_json::json!({
            "error": e
        }));
    }
}

async fn run() -> Result<(), String> {
    let msg = read_message()?;
    let bridge_config = ensure_bridge_config().await?;

    let action = msg
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Native message must specify an action.".to_owned())?;

    let endpoint = match action {
        "formats" => "/formats",
        "download" => "/downloads",
        "refresh" => "/downloads/refresh",
        "policy" | "capturePolicy" => "/policy",
        _ => return Err("Unsupported native action.".to_owned()),
    };

    let url = format!("http://127.0.0.1:{}{endpoint}", bridge_config.port);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let payload = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
    let resp = send_with_retry(&client, &url, &bridge_config, &payload).await?;

    let status = resp.status();
    let resp_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Could not read response from desktop app: {e}"))?;

    let body: serde_json::Value = serde_json::from_slice(&resp_bytes)
        .map_err(|e| format!("Invalid response from desktop app: {e}"))?;

    if !status.is_success() {
        let err_msg = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Desktop application error.");
        return Err(err_msg.to_owned());
    }

    write_message(&body)?;
    Ok(())
}

async fn send_with_retry(
    client: &reqwest::Client,
    url: &str,
    config: &BridgeConfig,
    payload: &[u8],
) -> Result<reqwest::Response, String> {
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
            try_launch_desktop();
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
            Err(format!("Could not connect to desktop app: {first_err}"))
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
    if let Some(exe) = find_desktop_executable() {
        let _ = std::process::Command::new(exe)
            .arg("--browser-download")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

async fn ensure_bridge_config() -> Result<BridgeConfig, String> {
    if let Ok(cfg) = load_bridge_config() {
        return Ok(cfg);
    }
    try_launch_desktop();
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if let Ok(cfg) = load_bridge_config() {
            return Ok(cfg);
        }
    }
    Err("Open Download Manager is not running. Launch the desktop app first.".to_owned())
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
