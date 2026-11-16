// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(cli): headless CLI supporting IDM-compatible switches (/d, /p, /f, /a, /n, /s) (W9)

use std::{
    path::PathBuf,
    time::Duration,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeConfig {
    port: u16,
    token: String,
    #[serde(default)]
    pid: Option<u32>,
}

#[derive(Debug, Default)]
struct CliArgs {
    url: Option<String>,
    path: Option<String>,
    filename: Option<String>,
    add_queue_only: bool,
    silent: bool,
    start_queue: bool,
    help: bool,
}

fn print_help() {
    println!("Open Download Manager CLI (open-download-manager-cli)");
    println!("IDM-compatible command line interface for scripts and automation.\n");
    println!("Usage:");
    println!("  open-download-manager-cli /d <url> [/p <path>] [/f <filename>] [/a] [/n] [/s]");
    println!("  open-download-manager-cli --url <url> [--path <path>] [--filename <name>] [--queue] [--silent]\n");
    println!("Switches:");
    println!("  /d, --url <url>          URL or magnet of the file to download");
    println!("  /p, --path <path>        Local directory to save the downloaded file");
    println!("  /f, --filename <name>    Custom filename to save as");
    println!("  /a, --queue              Add to queue without starting immediately");
    println!("  /s, --start-queue        Start queue processing");
    println!("  /n, --silent             Silent mode (suppress console status output)");
    println!("  /h, /?, --help           Display this help information");
}

fn parse_args() -> CliArgs {
    parse_args_from(std::env::args().skip(1))
}

fn parse_args_from<I: IntoIterator<Item = S>, S: AsRef<str>>(args: I) -> CliArgs {
    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let mut parsed = CliArgs::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        match arg.to_ascii_lowercase().as_str() {
            "/h" | "/?" | "-h" | "--help" => {
                parsed.help = true;
                return parsed;
            }
            "/d" | "-d" | "--url" => {
                if i + 1 < args.len() {
                    parsed.url = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "/p" | "-p" | "--path" => {
                if i + 1 < args.len() {
                    parsed.path = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "/f" | "-f" | "--filename" => {
                if i + 1 < args.len() {
                    parsed.filename = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "/a" | "-a" | "--queue" | "--add-queue" => {
                parsed.add_queue_only = true;
            }
            "/n" | "-n" | "--silent" => {
                parsed.silent = true;
            }
            "/s" | "-s" | "--start-queue" => {
                parsed.start_queue = true;
            }
            other => {
                if parsed.url.is_none() && (other.starts_with("http://") || other.starts_with("https://") || other.starts_with("magnet:")) {
                    parsed.url = Some(other.to_string());
                }
            }
        }
        i += 1;
    }

    parsed
}

fn load_bridge_config() -> Result<BridgeConfig, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Could not resolve user home directory.".to_owned())?;
    let path = home.join(".open-download-manager").join("bridge.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|_| "Open Download Manager is not running. Launch the desktop app first.".to_owned())?;
    serde_json::from_str(&content).map_err(|e| format!("Invalid bridge.json: {e}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();

    if args.help || (args.url.is_none() && !args.start_queue) {
        print_help();
        return Ok(());
    }

    let bridge = match load_bridge_config() {
        Ok(b) => b,
        Err(err) => {
            eprintln!("[odm-cli] Error: {err}");
            std::process::exit(1);
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    if let Some(ref url) = args.url {
        let is_magnet = url.starts_with("magnet:");
        let endpoint = if is_magnet {
            format!("http://127.0.0.1:{}/torrents", bridge.port)
        } else {
            format!("http://127.0.0.1:{}/downloads", bridge.port)
        };

        let payload = if is_magnet {
            serde_json::json!({
                "magnet": url,
            })
        } else {
            serde_json::json!({
                "action": "download",
                "url": url,
                "filename": args.filename,
                "directory": args.path,
                "startNow": !args.add_queue_only,
            })
        };

        let response = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", bridge.token))
            .header("Content-Type", "application/json")
            .header("Host", format!("127.0.0.1:{}", bridge.port))
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            if !args.silent {
                println!("[odm-cli] Successfully dispatched download: {url}");
                if let Some(f) = &args.filename {
                    println!("          Filename: {f}");
                }
                if let Some(p) = &args.path {
                    println!("          Destination: {p}");
                }
                if args.add_queue_only {
                    println!("          Status: Queued");
                } else {
                    println!("          Status: Starting");
                }
            }
        } else {
            let error_text = response.text().await.unwrap_or_default();
            eprintln!("[odm-cli] Dispatch failed: {error_text}");
            std::process::exit(1);
        }
    }

    if args.start_queue {
        let queue_endpoint = format!("http://127.0.0.1:{}/queue/start", bridge.port);
        let response = client
            .post(&queue_endpoint)
            .header("Authorization", format!("Bearer {}", bridge.token))
            .header("Content-Type", "application/json")
            .header("Host", format!("127.0.0.1:{}", bridge.port))
            .json(&serde_json::json!({}))
            .send()
            .await?;

        if response.status().is_success() {
            if !args.silent {
                println!("[odm-cli] Queue processing started.");
            }
        } else {
            let error_text = response.text().await.unwrap_or_default();
            eprintln!("[odm-cli] Failed to start queue: {error_text}");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_idm_style_switches() {
        let args = ["/d", "https://example.com/file.zip", "/p", "C:\\Downloads", "/f", "custom.zip", "/a", "/n"];
        let parsed = parse_args_from(args);
        assert_eq!(parsed.url.as_deref(), Some("https://example.com/file.zip"));
        assert_eq!(parsed.path.as_deref(), Some("C:\\Downloads"));
        assert_eq!(parsed.filename.as_deref(), Some("custom.zip"));
        assert!(parsed.add_queue_only);
        assert!(parsed.silent);
        assert!(!parsed.start_queue);
    }

    #[test]
    fn test_parse_gnu_style_switches() {
        let args = ["--url", "https://example.com/video.mp4", "--queue", "--start-queue"];
        let parsed = parse_args_from(args);
        assert_eq!(parsed.url.as_deref(), Some("https://example.com/video.mp4"));
        assert!(parsed.add_queue_only);
        assert!(parsed.start_queue);
        assert!(!parsed.silent);
    }

    #[test]
    fn test_parse_bare_url() {
        let args = ["magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709"];
        let parsed = parse_args_from(args);
        assert_eq!(
            parsed.url.as_deref(),
            Some("magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709")
        );
        assert!(!parsed.add_queue_only);
    }

    #[test]
    fn test_parse_help_switch() {
        let args = ["/h"];
        let parsed = parse_args_from(args);
        assert!(parsed.help);
    }
}
