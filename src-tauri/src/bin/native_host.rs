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
    let bridge_config = load_bridge_config()?;

    let action = msg
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Native message must specify an action.".to_owned())?;

    let endpoint = match action {
        "formats" => "/formats",
        "download" => "/downloads",
        _ => return Err("Unsupported native action.".to_owned()),
    };

    let url = format!("http://127.0.0.1:{}{endpoint}", bridge_config.port);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let payload = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", bridge_config.token))
        .header("Content-Type", "application/json")
        .header("Host", format!("127.0.0.1:{}", bridge_config.port))
        .body(payload)
        .send()
        .await
        .map_err(|e| format!("Could not connect to desktop app: {e}"))?;

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

fn load_bridge_config() -> Result<BridgeConfig, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Could not find user home directory.".to_owned())?;
    let path = home.join(".awesome-downloader").join("bridge.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|_| "Awesome Downloader is not running. Launch the desktop app first.".to_owned())?;
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
