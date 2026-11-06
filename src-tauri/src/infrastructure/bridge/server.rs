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
    },
    domain::{
        browser::{sanitize_browser_request, BrowserIncomingMessage},
        download::DownloadRequest,
        settings::Settings,
    },
    infrastructure::media::ytdlp::YtDlpTools,
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

        tokio::spawn(run_bridge_server(
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

        let expected_token = expected_token.clone();
        let transfer_service = transfer_service.clone();
        let settings = settings.clone();
        let ytdlp = ytdlp.clone();
        let on_download = on_download.clone();

        tokio::spawn(async move {
            handle_bridge_connection(
                &mut socket,
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
            match tools.fetch_formats(&parsed.url).await {
                Ok(items) => {
                    let _ = send_json_response(socket, 200, &serde_json::json!({"ok": true, "items": items})).await;
                }
                Err(e) => {
                    let _ = send_json_response(socket, 400, &serde_json::json!({"error": e.message})).await;
                }
            }
        } else {
            let _ = send_json_response(socket, 501, &serde_json::json!({"error": "yt-dlp is not available."})).await;
        }
        return;
    }

    if path == "/downloads" {
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

        let confirm = settings.confirm_browser_downloads;
        let review_only = !valid_browser.start_now && confirm;
        let queue_only = !valid_browser.start_now && confirm;
        let is_paired = valid_browser.audio_url.is_some();
        let is_youtube = valid_browser.youtube_format.is_some()
            || valid_browser.referer.as_ref().map_or(false, |r| r.contains("youtube.com") || r.contains("youtu.be"))
            || valid_browser.url.contains("youtube.com")
            || valid_browser.url.contains("youtu.be")
            || valid_browser.url.contains("googlevideo.com");
        let download_req = DownloadRequest {
            urls: valid_browser.urls.unwrap_or_else(|| vec![valid_browser.url]),
            filename: valid_browser.filename,
            directory: None,
            category: None,
            description: Some("Added via browser extension".to_owned()),
            queue_only,
            review_only: Some(review_only),
            scheduled_at: None,
            audio_url: valid_browser.audio_url,
            kind: if valid_browser.sabr.is_some() {
                Some("sabr".to_owned())
            } else if is_paired {
                Some("paired".to_owned())
            } else if is_youtube {
                Some("youtube".to_owned())
            } else {
                None
            },
            sabr: valid_browser.sabr,
            reuse_active: Some(true),
            referer: valid_browser.referer,
            youtube_format: valid_browser.youtube_format,
        };

        match transfer_service.add(download_req, &settings) {
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
