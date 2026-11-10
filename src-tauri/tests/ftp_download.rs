// Created: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Integration tests verifying ftp download functionality, edge cases, and error recovery.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_executor::{DownloadControl, DownloadExecutor, DownloadStopReason},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
    },
    domain::download::{
        BrowserRequestHeaders, DestinationAllocator, DownloadRecord, DownloadStatus,
        TransferValidator,
    },
    infrastructure::ftp::{
        FtpDownloadExecutor, TEMPORARY_DOWNLOAD_PREFIX,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::watch,
};

struct MockFtpServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    users: Arc<Mutex<HashMap<String, String>>>,
    reject_rest: Arc<AtomicBool>,
    reject_pasv: Arc<AtomicBool>,
    reject_epsv: Arc<AtomicBool>,
    reject_direct_nested_retr: Arc<AtomicBool>,
    abor_received: Arc<AtomicBool>,
    last_rest_offset: Arc<Mutex<Option<u64>>>,
}

impl MockFtpServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let files = Arc::new(Mutex::new(HashMap::new()));
        let users = Arc::new(Mutex::new(HashMap::new()));
        let reject_rest = Arc::new(AtomicBool::new(false));
        let reject_pasv = Arc::new(AtomicBool::new(false));
        let reject_epsv = Arc::new(AtomicBool::new(false));
        let reject_direct_nested_retr = Arc::new(AtomicBool::new(false));
        let abor_received = Arc::new(AtomicBool::new(false));
        let last_rest_offset = Arc::new(Mutex::new(None));

        let s_shutdown = shutdown.clone();
        let s_files = files.clone();
        let s_users = users.clone();
        let s_reject_rest = reject_rest.clone();
        let s_reject_pasv = reject_pasv.clone();
        let s_reject_epsv = reject_epsv.clone();
        let s_reject_nested = reject_direct_nested_retr.clone();
        let s_abor = abor_received.clone();
        let s_last_rest = last_rest_offset.clone();

        tokio::spawn(async move {
            while !s_shutdown.load(Ordering::SeqCst) {
                let (stream, _) = match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
                    Ok(Ok(val)) => val,
                    _ => continue,
                };

                let conn_files = s_files.clone();
                let conn_users = s_users.clone();
                let conn_reject_rest = s_reject_rest.clone();
                let conn_reject_pasv = s_reject_pasv.clone();
                let conn_reject_epsv = s_reject_epsv.clone();
                let conn_reject_nested = s_reject_nested.clone();
                let conn_abor = s_abor.clone();
                let conn_last_rest = s_last_rest.clone();

                tokio::spawn(async move {
                    handle_client(
                        stream,
                        conn_files,
                        conn_users,
                        conn_reject_rest,
                        conn_reject_pasv,
                        conn_reject_epsv,
                        conn_reject_nested,
                        conn_abor,
                        conn_last_rest,
                    )
                    .await;
                });
            }
        });

        Self {
            addr,
            shutdown,
            files,
            users,
            reject_rest,
            reject_pasv,
            reject_epsv,
            reject_direct_nested_retr,
            abor_received,
            last_rest_offset,
        }
    }

    fn add_file(&self, path: &str, content: Vec<u8>) {
        self.files.lock().unwrap().insert(path.to_owned(), content);
    }

    fn add_user(&self, user: &str, pass: &str) {
        self.users
            .lock()
            .unwrap()
            .insert(user.to_owned(), pass.to_owned());
    }

    fn stop(self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

async fn handle_client(
    stream: TcpStream,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    users: Arc<Mutex<HashMap<String, String>>>,
    reject_rest: Arc<AtomicBool>,
    reject_pasv: Arc<AtomicBool>,
    reject_epsv: Arc<AtomicBool>,
    reject_direct_nested: Arc<AtomicBool>,
    abor_received: Arc<AtomicBool>,
    last_rest_offset: Arc<Mutex<Option<u64>>>,
) {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // Send greeting
    if write_half.write_all(b"220 Mock FTP Server Ready\r\n").await.is_err() {
        return;
    }
    let _ = write_half.flush().await;

    let mut current_user_attempt: Option<String> = None;
    let mut current_dir = String::new();
    let mut pasv_listener: Option<TcpListener> = None;
    let mut active_client_addr: Option<SocketAddr> = None;
    let mut rest_offset: u64 = 0;

    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        let mut parts = trimmed.splitn(2, ' ');
        let command = parts.next().unwrap_or("").to_ascii_uppercase();
        let arg = parts.next().unwrap_or("");

        match command.as_str() {
            "FEAT" => {
                let feat_msg = b"211-Features:\r\n EPSV\r\n EPRT\r\n SIZE\r\n MDTM\r\n REST STREAM\r\n MLST size*;modify*;\r\n UTF8\r\n211 End\r\n";
                let _ = write_half.write_all(feat_msg).await;
            }
            "OPTS" => {
                let _ = write_half.write_all(b"200 OPTS command successful\r\n").await;
            }
            "USER" => {
                current_user_attempt = Some(arg.to_owned());
                if arg == "anonymous" {
                    let _ = write_half.write_all(b"230 User logged in, proceed\r\n").await;
                } else {
                    let _ = write_half.write_all(b"331 User name okay, need password\r\n").await;
                }
            }
            "PASS" => {
                if let Some(ref user) = current_user_attempt {
                    let valid_pass = users.lock().unwrap().get(user).cloned();
                    if valid_pass.as_deref() == Some(arg) || user == "anonymous" {
                        let _ = write_half.write_all(b"230 Password accepted\r\n").await;
                    } else {
                        let _ = write_half.write_all(b"530 Not logged in\r\n").await;
                    }
                } else {
                    let _ = write_half.write_all(b"503 Bad sequence of commands\r\n").await;
                }
            }
            "TYPE" => {
                let _ = write_half.write_all(b"200 Type set to I\r\n").await;
            }
            "SIZE" => {
                let maybe_len = {
                    let file_guard = files.lock().unwrap();
                    let clean_arg = arg.trim_start_matches('/');
                    file_guard.get(arg).or_else(|| file_guard.get(clean_arg)).map(|d| d.len())
                };
                if let Some(len) = maybe_len {
                    let resp = format!("213 {len}\r\n");
                    let _ = write_half.write_all(resp.as_bytes()).await;
                } else {
                    let _ = write_half.write_all(b"550 File not found\r\n").await;
                }
            }
            "MDTM" => {
                let exists = {
                    let file_guard = files.lock().unwrap();
                    let clean_arg = arg.trim_start_matches('/');
                    file_guard.contains_key(arg) || file_guard.contains_key(clean_arg)
                };
                if exists {
                    let _ = write_half.write_all(b"213 20261108030000\r\n").await;
                } else {
                    let _ = write_half.write_all(b"550 File not found\r\n").await;
                }
            }
            "MLST" => {
                let maybe_len = {
                    let file_guard = files.lock().unwrap();
                    let clean_arg = arg.trim_start_matches('/');
                    file_guard.get(arg).or_else(|| file_guard.get(clean_arg)).map(|d| d.len())
                };
                if let Some(len) = maybe_len {
                    let resp = format!("250- Listing {arg}\r\n type=file;size={len};modify=20261108030000; {arg}\r\n250 End\r\n");
                    let _ = write_half.write_all(resp.as_bytes()).await;
                } else {
                    let _ = write_half.write_all(b"550 File not found\r\n").await;
                }
            }
            "CWD" => {
                current_dir = arg.trim_matches('/').to_owned();
                let _ = write_half.write_all(b"250 Directory changed\r\n").await;
            }
            "EPSV" => {
                if reject_epsv.load(Ordering::SeqCst) || reject_pasv.load(Ordering::SeqCst) {
                    let _ = write_half.write_all(b"500 EPSV not supported\r\n").await;
                } else {
                    match TcpListener::bind("127.0.0.1:0").await {
                        Ok(l) => {
                            let port = l.local_addr().unwrap().port();
                            let resp = format!("229 Entering Extended Passive Mode (|||{port}|)\r\n");
                            pasv_listener = Some(l);
                            let _ = write_half.write_all(resp.as_bytes()).await;
                        }
                        Err(_) => {
                            let _ = write_half.write_all(b"425 Cannot open data connection\r\n").await;
                        }
                    }
                }
            }
            "PASV" => {
                if reject_pasv.load(Ordering::SeqCst) {
                    let _ = write_half.write_all(b"500 PASV not supported\r\n").await;
                } else {
                    match TcpListener::bind("127.0.0.1:0").await {
                        Ok(l) => {
                            let port = l.local_addr().unwrap().port();
                            let p1 = port / 256;
                            let p2 = port % 256;
                            let resp = format!("227 Entering Passive Mode (127,0,0,1,{p1},{p2})\r\n");
                            pasv_listener = Some(l);
                            let _ = write_half.write_all(resp.as_bytes()).await;
                        }
                        Err(_) => {
                            let _ = write_half.write_all(b"425 Cannot open data connection\r\n").await;
                        }
                    }
                }
            }
            "PORT" => {
                let nums: Vec<u8> = arg.split(',').filter_map(|s| s.parse().ok()).collect();
                if nums.len() == 6 {
                    let ip = std::net::Ipv4Addr::new(nums[0], nums[1], nums[2], nums[3]);
                    let port = ((nums[4] as u16) << 8) | (nums[5] as u16);
                    active_client_addr = Some(SocketAddr::new(std::net::IpAddr::V4(ip), port));
                    let _ = write_half.write_all(b"200 PORT command successful\r\n").await;
                } else {
                    let _ = write_half.write_all(b"501 Syntax error in parameters\r\n").await;
                }
            }
            "REST" => {
                if reject_rest.load(Ordering::SeqCst) {
                    let _ = write_half.write_all(b"501 Cannot REST\r\n").await;
                } else {
                    let offset: u64 = arg.parse().unwrap_or(0);
                    rest_offset = offset;
                    *last_rest_offset.lock().unwrap() = Some(offset);
                    let resp = format!("350 Restarting at {offset}\r\n");
                    let _ = write_half.write_all(resp.as_bytes()).await;
                }
            }
            "RETR" => {
                if reject_direct_nested.load(Ordering::SeqCst) && arg.contains('/') && current_dir.is_empty() {
                    // Simulate a server that requires CWD before RETR
                    let _ = write_half.write_all(b"550 Direct nested path not permitted. Use CWD first\r\n").await;
                    let _ = write_half.flush().await;
                    line.clear();
                    continue;
                }

                let matched = {
                    let file_guard = files.lock().unwrap();
                    let clean_arg = arg.trim_start_matches('/');
                    let full_scoped = if !current_dir.is_empty() && !clean_arg.starts_with(&current_dir) {
                        format!("{current_dir}/{clean_arg}")
                    } else {
                        clean_arg.to_owned()
                    };

                    file_guard.get(arg)
                        .or_else(|| file_guard.get(clean_arg))
                        .or_else(|| file_guard.get(&full_scoped))
                        .cloned()
                };

                if let Some(content) = matched {
                    let _ = write_half.write_all(b"150 Opening BINARY mode data connection\r\n").await;
                    let _ = write_half.flush().await;

                    let data_stream = if let Some(listener) = pasv_listener.take() {
                        match listener.accept().await {
                            Ok((s, _)) => Some(s),
                            Err(_) => None,
                        }
                    } else if let Some(addr) = active_client_addr.take() {
                        TcpStream::connect(addr).await.ok()
                    } else {
                        None
                    };

                    if let Some(mut stream) = data_stream {
                        let offset = rest_offset as usize;
                        let slice = if offset < content.len() {
                            &content[offset..]
                        } else {
                            &[]
                        };
                        let _ = stream.write_all(slice).await;
                        let _ = stream.flush().await;
                        drop(stream);
                        rest_offset = 0;
                        let _ = write_half.write_all(b"226 Transfer complete\r\n").await;
                    } else {
                        let _ = write_half.write_all(b"425 Can't open data connection\r\n").await;
                    }
                } else {
                    let _ = write_half.write_all(b"550 File not found\r\n").await;
                }
            }
            "ABOR" => {
                abor_received.store(true, Ordering::SeqCst);
                let _ = write_half.write_all(b"226 ABOR command successful\r\n").await;
            }
            "QUIT" => {
                let _ = write_half.write_all(b"221 Goodbye\r\n").await;
                let _ = write_half.flush().await;
                break;
            }
            _ => {
                let _ = write_half.write_all(b"500 Command unrecognized\r\n").await;
            }
        }
        let _ = write_half.flush().await;
        line.clear();
    }
}

fn create_test_record(id: &str, url: &str, filename: &str, dir: &str) -> DownloadRecord {
    DownloadRecord {
        id: id.to_owned(),
        url: url.to_owned(),
        filename: filename.to_owned(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-11-08T03:00:00Z".to_owned(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        directory: Some(dir.to_owned()),
        path: None,
        temp_directory: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("ftp".to_owned()),
        resumable: Some(true),
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        audio_url: None,
        referer: None,
        youtube_format: None,
        transfer_kind: Some("ftp".to_owned()),
        etag: None,
        validator: None,
        conflict_policy: None,
        sabr: None,
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
        completion_actions: None,
        action_ledger: Vec::new(),
    }
}

#[tokio::test]
async fn test_anonymous_login_and_passive_download() {
    let server = MockFtpServer::start().await;
    let data = b"Hello, this is a test FTP file content for anonymous transfer!".to_vec();
    server.add_file("hello.txt", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "anon-1",
        &format!("ftp://{}/hello.txt", server.addr),
        "hello.txt",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let pe = progress_events.clone();
    let handler = Arc::new(move |p: DownloadProgress| {
        pe.lock().unwrap().push(p);
    });

    let res = executor.execute(record, handler, control, limiter).await.expect("Transfer should succeed");

    assert_eq!(res.downloaded, data.len() as u64);
    assert_eq!(res.total, Some(data.len() as u64));

    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_authenticated_login_and_passive_download() {
    let server = MockFtpServer::start().await;
    server.add_user("testuser", "secretpass");
    let data = b"Confidential document content for authenticated transfer.".to_vec();
    server.add_file("secure.pdf", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "auth-1",
        &format!("ftp://testuser:secretpass@{}/secure.pdf", server.addr),
        "secure.pdf",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("Authenticated transfer should succeed");

    assert_eq!(res.downloaded, data.len() as u64);
    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_bad_credentials_returns_ftp_auth_failed() {
    let server = MockFtpServer::start().await;
    server.add_user("testuser", "secretpass");
    server.add_file("secure.pdf", b"data".to_vec());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "bad-auth-1",
        &format!("ftp://testuser:wrongpass@{}/secure.pdf", server.addr),
        "secure.pdf",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let err = executor.execute(record, handler, control, limiter).await.expect_err("Should fail with auth error");
    assert_eq!(err.code, "ftp_auth_failed");

    server.stop();
}

#[tokio::test]
async fn test_resume_with_rest_negotiation() {
    let server = MockFtpServer::start().await;
    let data: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
    server.add_file("large.bin", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    // Pre-create partial file with first 75 bytes
    let temp_file = temp_dir.path().join(format!("{TEMPORARY_DOWNLOAD_PREFIX}resume-1.part"));
    tokio::fs::write(&temp_file, &data[..75]).await.unwrap();

    let mut record = create_test_record(
        "resume-1",
        &format!("ftp://{}/large.bin", server.addr),
        "large.bin",
        temp_dir.path().to_str().unwrap(),
    );
    record.downloaded = 75;
    record.total = Some(200);
    record.validator = Some(TransferValidator::LastModified("20261108030000".to_owned()));

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("Resume transfer should succeed");

    assert_eq!(res.downloaded, 200);
    assert_eq!(*server.last_rest_offset.lock().unwrap(), Some(75));

    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_resume_rejected_falls_back_to_clean_restart() {
    let server = MockFtpServer::start().await;
    server.reject_rest.store(true, Ordering::SeqCst);
    let data: Vec<u8> = (0..150).map(|i| (i % 256) as u8).collect();
    server.add_file("restart.bin", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    // Pre-create partial file with first 50 corrupted/stale bytes
    let temp_file = temp_dir.path().join(format!("{TEMPORARY_DOWNLOAD_PREFIX}restart-1.part"));
    tokio::fs::write(&temp_file, vec![0xFF; 50]).await.unwrap();

    let mut record = create_test_record(
        "restart-1",
        &format!("ftp://{}/restart.bin", server.addr),
        "restart.bin",
        temp_dir.path().to_str().unwrap(),
    );
    record.downloaded = 50;
    record.total = Some(150);
    record.validator = Some(TransferValidator::LastModified("20261108030000".to_owned()));

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("Restart transfer should succeed");

    assert_eq!(res.downloaded, 150);
    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_file_metadata_changed_triggers_clean_restart() {
    let server = MockFtpServer::start().await;
    let data = b"New updated file data after server side replacement".to_vec();
    server.add_file("changed.bin", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    // Pre-create partial file with old length and old validator
    let temp_file = temp_dir.path().join(format!("{TEMPORARY_DOWNLOAD_PREFIX}changed-1.part"));
    tokio::fs::write(&temp_file, vec![0xAA; 30]).await.unwrap();

    let mut record = create_test_record(
        "changed-1",
        &format!("ftp://{}/changed.bin", server.addr),
        "changed.bin",
        temp_dir.path().to_str().unwrap(),
    );
    record.downloaded = 30;
    record.total = Some(999); // Server actual size is data.len() != 999
    record.validator = Some(TransferValidator::LastModified("20200101000000".to_owned()));

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("Transfer should succeed via clean restart");

    assert_eq!(res.downloaded, data.len() as u64);
    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_active_mode_fallback_when_pasv_unsupported() {
    let server = MockFtpServer::start().await;
    server.reject_pasv.store(true, Ordering::SeqCst);
    server.reject_epsv.store(true, Ordering::SeqCst);
    let data = b"Active mode fallback data transfer successfully executed!".to_vec();
    server.add_file("active.txt", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "active-1",
        &format!("ftp://{}/active.txt", server.addr),
        "active.txt",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("Active mode fallback should succeed");

    assert_eq!(res.downloaded, data.len() as u64);
    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_midstream_cancellation_preserves_partial_file() {
    let server = MockFtpServer::start().await;
    let data: Vec<u8> = (0..50000).map(|i| (i % 256) as u8).collect();
    server.add_file("big.bin", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "cancel-1",
        &format!("ftp://{}/big.bin", server.addr),
        "big.bin",
        temp_dir.path().to_str().unwrap(),
    );

    let (tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    // Limit speed to 10 KB/s so we can cancel cleanly midstream
    let limiter = Arc::new(DownloadRateLimiter::new(10));

    let tx_clone = tx.clone();
    let handler = Arc::new(move |p: DownloadProgress| {
        if p.downloaded > 1000 {
            let _ = tx_clone.send(Some(DownloadStopReason::Pause));
        }
    });

    let res = executor.execute(record.clone(), handler, control, limiter).await;
    assert!(res.is_err());
    let err = res.err().unwrap();
    assert_eq!(err.code, "download_paused");

    let temp_file = temp_dir.path().join(format!("{TEMPORARY_DOWNLOAD_PREFIX}cancel-1.part"));
    assert!(tokio::fs::try_exists(&temp_file).await.unwrap(), "Partial file must be preserved");

    // Test discard partial
    executor.discard_partial(&record).expect("Discard should succeed");
    assert!(!tokio::fs::try_exists(&temp_file).await.unwrap(), "Partial file must be removed after discard");

    assert!(server.abor_received.load(Ordering::SeqCst), "ABOR command must be sent on cancel");

    server.stop();
}

#[tokio::test]
async fn test_file_not_found_mapped_to_actionable_error() {
    let server = MockFtpServer::start().await;

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "fnf-1",
        &format!("ftp://{}/missing_file.bin", server.addr),
        "missing_file.bin",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let err = executor.execute(record, handler, control, limiter).await.expect_err("Should return file not found");
    assert_eq!(err.code, "ftp_file_not_found");

    server.stop();
}

#[tokio::test]
async fn test_extended_passive_mode_epsv_download() {
    let server = MockFtpServer::start().await;
    let data = b"Testing modern RFC 2428 EPSV transfer mode!".to_vec();
    server.add_file("epsv_test.txt", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "epsv-1",
        &format!("ftp://{}/epsv_test.txt", server.addr),
        "epsv_test.txt",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("EPSV transfer should succeed");
    assert_eq!(res.downloaded, data.len() as u64);

    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_cwd_directory_navigation_fallback() {
    let server = MockFtpServer::start().await;
    // Tell server to reject direct RETR on nested paths so CWD fallback is triggered
    server.reject_direct_nested_retr.store(true, Ordering::SeqCst);
    let data = b"Nested document retrieved via CWD fallback navigation.".to_vec();
    server.add_file("nested/subfolder/document.pdf", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "cwd-1",
        &format!("ftp://{}/nested/subfolder/document.pdf", server.addr),
        "document.pdf",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("CWD fallback transfer should succeed");
    assert_eq!(res.downloaded, data.len() as u64);

    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}

#[tokio::test]
async fn test_encoded_spaces_and_special_characters_download() {
    let server = MockFtpServer::start().await;
    let data = b"File with spaces and parenthesis in filename and directory".to_vec();
    server.add_file("Special Dir/Report (2026).pdf", data.clone());

    let temp_dir = tempfile::tempdir().unwrap();
    let executor = FtpDownloadExecutor::new(DestinationAllocator::new());

    let record = create_test_record(
        "special-1",
        &format!("ftp://{}/Special%20Dir/Report%20(2026).pdf", server.addr),
        "Report (2026).pdf",
        temp_dir.path().to_str().unwrap(),
    );

    let (_, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let limiter = Arc::new(DownloadRateLimiter::new(0));
    let handler = Arc::new(|_| {});

    let res = executor.execute(record, handler, control, limiter).await.expect("Special char transfer should succeed");
    assert_eq!(res.downloaded, data.len() as u64);

    let content = tokio::fs::read(&res.path).await.unwrap();
    assert_eq!(content, data);

    server.stop();
}
