// Created: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Reason: fix(browser): classify browser m3u8 requests as HLS even when audio_url is present
// Description: Local loopback HTTP bridge server receiving intercepted downloads from browser extensions.

use std::{
    path::PathBuf,
    sync::Arc,
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::watch,
};

use crate::{
    application::{
        download_transfer_service::DownloadTransferService,
        error::{AppError, AppResult},
        settings_service::SettingsRepository,
    },
    domain::{
        browser::{sanitize_browser_request, BrowserIncomingMessage},
        download::DownloadRequest,
        settings::Settings,
    },
    infrastructure::{media::ytdlp::YtDlpTools, settings_store::SettingsStore},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub port: u16,
    pub token: String,
}

pub type OnDownloadCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

pub struct LoopbackBridge {
    pub config: BridgeConfig,
    stop_sender: watch::Sender<bool>,
    data_directory: PathBuf,
}

impl LoopbackBridge {
    pub async fn start(
        data_directory: PathBuf,
        transfer_service: DownloadTransferService,
        settings: Settings,
        on_download: Option<OnDownloadCallback>,
    ) -> AppResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| AppError::bridge(format!("Could not bind loopback bridge: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| AppError::bridge(format!("Could not get local address: {e}")))?
            .port();

        let token = generate_token();
        let config = BridgeConfig {
            port,
            token: token.clone(),
        };

        tokio::fs::create_dir_all(&data_directory)
            .await
            .map_err(|e| AppError::bridge(format!("Could not create bridge folder: {e}")))?;
        let config_path = data_directory.join("bridge.json");

        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| AppError::bridge(format!("Could not serialize bridge config: {e}")))?;
        tokio::fs::write(&config_path, config_json)
            .await
            .map_err(|e| AppError::bridge(format!("Could not write bridge.json: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600));
        }

        let (stop_sender, stop_receiver) = watch::channel(false);
        let transfer_service = Arc::new(transfer_service);
        let settings = Arc::new(settings);
        let ytdlp = YtDlpTools::new().map(Arc::new);
        let data_dir_arc = Arc::new(data_directory.clone());

        tokio::spawn(run_bridge_server(
            data_dir_arc,
            listener,
            port,
            token,
            stop_receiver,
            transfer_service,
            settings,
            ytdlp,
            on_download,
        ));

        Ok(Self {
            config,
            stop_sender,
            data_directory,
        })
    }

    pub async fn stop(self) {
        let _ = self.stop_sender.send(true);
        let config_path = self.data_directory.join("bridge.json");
        let _ = tokio::fs::remove_file(config_path).await;
    }
}

async fn run_bridge_server(
    data_directory: Arc<PathBuf>,
    listener: TcpListener,
    port: u16,
    expected_token: String,
    mut stop_receiver: watch::Receiver<bool>,
    transfer_service: Arc<DownloadTransferService>,
    settings: Arc<Settings>,
    ytdlp: Option<Arc<YtDlpTools>>,
    on_download: Option<OnDownloadCallback>,
) {
    loop {
        let (mut socket, _) = tokio::select! {
            _ = stop_receiver.changed() => break,
            accept_res = listener.accept() => {
                match accept_res {
                    Ok(conn) => conn,
                    Err(_) => continue,
                }
            }
        };

        let data_directory = data_directory.clone();
        let expected_token = expected_token.clone();
        let transfer_service = transfer_service.clone();
        let settings = settings.clone();
        let ytdlp = ytdlp.clone();
        let on_download = on_download.clone();

        tokio::spawn(async move {
            handle_bridge_connection(
                &mut socket,
                data_directory,
                port,
                &expected_token,
                transfer_service,
                settings,
                ytdlp,
                on_download,
            )
            .await;
        });
    }
}

async fn handle_bridge_connection(
    socket: &mut tokio::net::TcpStream,
    data_directory: Arc<PathBuf>,
    port: u16,
    expected_token: &str,
    transfer_service: Arc<DownloadTransferService>,
    settings: Arc<Settings>,
    ytdlp: Option<Arc<YtDlpTools>>,
    on_download: Option<OnDownloadCallback>,
) {
    let mut header_buf = [0u8; 8192];
    let mut total_read = 0;
    let header_end;

    loop {
        let n = match socket.read(&mut header_buf[total_read..]).await {
            Ok(n) if n > 0 => n,
            _ => return,
        };
        total_read += n;
        if let Some(pos) = header_buf[..total_read]
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
        {
            header_end = pos + 4;
            break;
        }
        if total_read == header_buf.len() {
            let _ = send_json_response(socket, 413, &serde_json::json!({"error": "Request headers too large."})).await;
            return;
        }
    }

    let header_str = String::from_utf8_lossy(&header_buf[..header_end]);
    let mut lines = header_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let mut origin = None;
    let mut host = None;
    let mut auth = None;
    let mut content_type = None;
    let mut content_length = 0_usize;

    for line in lines {
        if let Some((name, val)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let val = val.trim();
            match name.as_str() {
                "origin" => origin = Some(val.to_owned()),
                "host" => host = Some(val.to_owned()),
                "authorization" => auth = Some(val.to_owned()),
                "content-type" => content_type = Some(val.to_owned()),
                "content-length" => content_length = val.parse::<usize>().unwrap_or(0),
                _ => {}
            }
        }
    }

    // Origin header must NOT be present
    if origin.is_some() {
        let _ = send_json_response(socket, 403, &serde_json::json!({"error": "Forbidden origin or host."})).await;
        return;
    }

    // Host must match 127.0.0.1:{port}
    let expected_host = format!("127.0.0.1:{port}");
    if host.as_deref() != Some(&expected_host) {
        let _ = send_json_response(socket, 403, &serde_json::json!({"error": "Forbidden origin or host."})).await;
        return;
    }

    // Check authorization: Bearer {expected_token}
    let expected_auth = format!("Bearer {expected_token}");
    if !constant_time_compare(auth.as_deref().unwrap_or("").as_bytes(), expected_auth.as_bytes()) {
        let _ = send_json_response(socket, 401, &serde_json::json!({"error": "Unauthorized."})).await;
        return;
    }

    if method == "GET" && path == "/health" {
        let _ = send_json_response(socket, 200, &serde_json::json!({"ok": true, "version": "0.1.0"})).await;
        return;
    }

    let active_settings = if data_directory.join("settings.json").exists() {
        SettingsStore::new(&*data_directory)
            .load(settings.default_directory.clone())
            .unwrap_or_else(|_| (*settings).clone())
    } else {
        (*settings).clone()
    };

    if (method == "GET" || method == "POST") && path == "/policy" {
        let _ = send_json_response(socket, 200, &serde_json::json!({
            "ok": true,
            "policy": active_settings.capture_policy,
        })).await;
        return;
    }

    if method != "POST" || !matches!(path, "/downloads" | "/formats") {
        let _ = send_json_response(socket, 404, &serde_json::json!({"error": "Not found."})).await;
        return;
    }

    if !content_type.as_deref().unwrap_or("").starts_with("application/json") {
        let _ = send_json_response(socket, 415, &serde_json::json!({"error": "JSON required."})).await;
        return;
    }

    if content_length > 1024 * 1024 {
        let _ = send_json_response(socket, 413, &serde_json::json!({"error": "Request too large."})).await;
        return;
    }

    // Read remaining body bytes
    let body_start = header_end;
    let initial_body_bytes = total_read - body_start;
    let mut body = Vec::with_capacity(content_length);
    if initial_body_bytes > 0 {
        body.extend_from_slice(&header_buf[body_start..total_read]);
    }

    while body.len() < content_length {
        let mut buf = [0u8; 4096];
        let needed = (content_length - body.len()).min(buf.len());
        let n = match socket.read(&mut buf[..needed]).await {
            Ok(n) if n > 0 => n,
            _ => break,
        };
        body.extend_from_slice(&buf[..n]);
    }

    if body.len() != content_length {
        let _ = send_json_response(socket, 400, &serde_json::json!({"error": "Incomplete request body."})).await;
        return;
    }

    if path == "/formats" {
        #[derive(Deserialize)]
        struct FormatsReq {
            url: String,
        }
        let parsed: FormatsReq = match serde_json::from_slice(&body) {
            Ok(p) => p,
            Err(_) => {
                let _ = send_json_response(socket, 400, &serde_json::json!({"error": "Invalid JSON body."})).await;
                return;
            }
        };

        if let Some(tools) = ytdlp {
            match tools.fetch_site_formats(&parsed.url).await {
                Ok(items) => {
                    let _ = send_json_response(socket, 200, &serde_json::json!({"ok": true, "items": items})).await;
                }
                Err(_) => {
                    let _ = send_json_response(socket, 200, &serde_json::json!({"ok": true, "items": []})).await;
                }
            }
        } else {
            let _ = send_json_response(socket, 200, &serde_json::json!({"ok": true, "items": []})).await;
        }
        return;
    }

    if path == "/downloads" || path == "/downloads/refresh" {
        let msg: BrowserIncomingMessage = match serde_json::from_slice(&body) {
            Ok(m) => m,
            Err(e) => {
                let _ = send_json_response(socket, 400, &serde_json::json!({"error": format!("Invalid JSON: {e}")})).await;
                return;
            }
        };

        let valid_browser = match sanitize_browser_request(msg) {
            Ok(v) => v,
            Err(e) => {
                let _ = send_json_response(socket, 400, &serde_json::json!({"error": e})).await;
                return;
            }
        };

        if valid_browser.action == "refresh" || path == "/downloads/refresh" {
            if let Some(ref target_id) = valid_browser.download_id {
                let norm_sabr = valid_browser.sabr.as_ref().and_then(|s| crate::domain::media::sabr::normalize_sabr_session(s).ok());
                match transfer_service.refresh_download(
                    target_id,
                    Some(valid_browser.url.clone()),
                    valid_browser.audio_url.clone(),
                    norm_sabr,
                ) {
                    Ok(id) => {
                        let _ = send_json_response(socket, 200, &serde_json::json!({"ok": true, "id": id})).await;
                    }
                    Err(e) => {
                        let _ = send_json_response(socket, 400, &serde_json::json!({"error": e.message})).await;
                    }
                }
                return;
            }
        }

        let decision = active_settings.capture_policy.evaluate(&valid_browser.url, valid_browser.filename.as_deref());
        match &decision {
            crate::domain::browser::CaptureDecision::ExcludedHost(reason)
            | crate::domain::browser::CaptureDecision::ExcludedAddress(reason) => {
                let _ = send_json_response(
                    socket,
                    403,
                    &serde_json::json!({
                        "error": format!("Download excluded by browser capture policy: {reason}"),
                        "decision": decision.decision_str(),
                    }),
                )
                .await;
                return;
            }
            _ => {}
        }

        let is_confirmation_required = matches!(decision, crate::domain::browser::CaptureDecision::ConfirmationRequired(_));
        let confirm = active_settings.confirm_browser_downloads || is_confirmation_required;
        let review_only = (!valid_browser.start_now || is_confirmation_required) && confirm;
        let queue_only = review_only;
        let is_hls = valid_browser.url.ends_with(".m3u8")
            || valid_browser.url.contains(".m3u8?");
        let is_paired = !is_hls && valid_browser.audio_url.is_some();
        let is_raw_youtube_page = crate::domain::media::youtube_url(&valid_browser.url).is_some();
        let primary_url = valid_browser.url.clone();
        let referer = valid_browser.canonical_url.clone().or(valid_browser.referer.clone());
        let kind = if valid_browser.sabr.is_some() {
            Some("sabr".to_owned())
        } else if is_hls {
            Some("hls".to_owned())
        } else if is_paired {
            Some("paired".to_owned())
        } else if is_raw_youtube_page {
            Some("youtube".to_owned())
        } else {
            None
        };
        let download_req = DownloadRequest {
            urls: valid_browser.urls.unwrap_or_else(|| vec![primary_url]),
            filename: valid_browser.filename,
            directory: None,
            temp_directory: None,
            category: None,
            description: Some("Added via browser extension".to_owned()),
            queue_only,
            review_only: Some(review_only),
            scheduled_at: None,
            queue_id: None,
            audio_url: valid_browser.audio_url,
            kind,
            sabr: valid_browser.sabr,
            reuse_active: Some(true),
            referer,
            youtube_format: valid_browser.youtube_format,
            conflict_policy: None,
            credentials: None,
            completion_actions: None,
        };

        match transfer_service.add(download_req, &active_settings) {
            Ok(ids) => {
                if let Some(ref cb) = on_download {
                    if let Some(first_id) = ids.first() {
                        cb(first_id.clone());
                    }
                }
                let _ = send_json_response(socket, 201, &serde_json::json!({"ok": true, "ids": ids})).await;
            }
            Err(e) => {
                let _ = send_json_response(socket, 400, &serde_json::json!({"error": e.message})).await;
            }
        }
    }
}

async fn send_json_response(
    socket: &mut tokio::net::TcpStream,
    status_code: u16,
    value: &serde_json::Value,
) -> std::io::Result<()> {
    let body = serde_json::to_vec(value).unwrap_or_default();
    let reason = match status_code {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        _ => "Response",
    };

    let headers = format!(
        "HTTP/1.1 {status_code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );

    socket.write_all(headers.as_bytes()).await?;
    socket.write_all(&body).await?;
    socket.flush().await
}

fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn generate_token() -> String {
    let u1 = uuid::Uuid::new_v4();
    let u2 = uuid::Uuid::new_v4();
    format!("{}{}", u1.simple(), u2.simple())
}
