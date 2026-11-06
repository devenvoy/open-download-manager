use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::Command,
    sync::Arc,
    thread,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use open_download_manager_lib::{
    domain::media_tools::{MediaToolType, ToolResolutionSource},
    infrastructure::media::{
        download_spec_for, find_tool_path, HttpToolDownloaderClient, LinuxAdapter, MacosAdapter,
        MediaToolAdapter, MediaToolsManager, ToolDownloaderClient, ToolPathResolver, ToolPlatform,
        UnsupportedAdapter, WindowsAdapter,
    },
};

fn temporary_sandbox() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("open-download-manager-sandbox-")
        .tempdir()
        .expect("temporary sandbox directory")
}

fn host_executable_name(tool: MediaToolType) -> &'static str {
    ToolPathResolver::new().adapter().executable_name(tool)
}

fn sample_executable_script() -> &'static [u8] {
    #[cfg(windows)]
    {
        b"@echo off\r\necho sandbox-media-tool-ok\r\n"
    }
    #[cfg(not(windows))]
    {
        b"#!/bin/sh\necho \"sandbox-media-tool-ok\"\nexit 0\n"
    }
}

fn serve_sandbox_binary(status: &'static str, body: Vec<u8>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("sandbox TCP listener should bind");
    let address = listener.local_addr().expect("sandbox TCP listener address");

    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let headers = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(headers.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });

    (format!("http://{address}/binary"), handle)
}

fn serve_truncated_binary(header_len: usize, body: Vec<u8>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("sandbox TCP listener should bind");
    let address = listener.local_addr().expect("sandbox TCP listener address");

    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {header_len}\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(headers.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
            // Drop stream immediately to trigger unexpected EOF
        }
    });

    (format!("http://{address}/truncated"), handle)
}

#[test]
fn test_media_tool_type_parsing_and_names() {
    assert_eq!(MediaToolType::parse("ffmpeg"), Some(MediaToolType::Ffmpeg));
    assert_eq!(MediaToolType::parse("FFMPEG"), Some(MediaToolType::Ffmpeg));
    assert_eq!(MediaToolType::parse("yt-dlp"), Some(MediaToolType::YtDlp));
    assert_eq!(MediaToolType::parse("ytdlp"), Some(MediaToolType::YtDlp));
    assert_eq!(MediaToolType::parse("unknown"), None);

    assert_eq!(MediaToolType::Ffmpeg.name(), "ffmpeg");
    assert_eq!(MediaToolType::YtDlp.name(), "yt-dlp");
    assert_eq!(MediaToolType::Ffmpeg.env_var_override(), "AD_FFMPEG_PATH");
    assert_eq!(MediaToolType::YtDlp.env_var_override(), "AD_YTDLP_PATH");
}

#[tokio::test]
async fn test_real_sandbox_http_download_and_execution() {
    let sandbox = temporary_sandbox();
    let dest_path = sandbox.path().join("sandbox_tool_bin");

    let script = sample_executable_script().to_vec();
    let (url, server_handle) = serve_sandbox_binary("200 OK", script);

    let client = HttpToolDownloaderClient::default();
    let result = client.download_binary(&url, &dest_path).await;
    server_handle
        .join()
        .expect("server thread terminates cleanly");

    assert!(
        result.is_ok(),
        "Download should succeed: {:?}",
        result.err()
    );
    assert!(dest_path.exists(), "Target binary must exist in sandbox");

    // The downloader is transport-only; the platform adapter finalizes the binary.
    ToolPathResolver::new()
        .adapter()
        .finalize_binary(&dest_path)
        .expect("finalize downloaded binary");

    // Verify temp file was atomically removed
    let temp_dest = dest_path.with_extension("download.tmp");
    assert!(
        !temp_dest.exists(),
        "Temporary file must not linger in sandbox"
    );

    // Verify Unix executable permissions
    #[cfg(unix)]
    {
        let metadata = std::fs::metadata(&dest_path).expect("file metadata");
        let permissions = metadata.permissions();
        assert_ne!(
            permissions.mode() & 0o111,
            0,
            "Downloaded file must have executable permissions set (0o755)"
        );
    }

    // Real sandbox process execution
    let output = Command::new(&dest_path)
        .output()
        .expect("Spawn sandbox executable");

    assert!(output.status.success(), "Executable must exit 0 in sandbox");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("sandbox-media-tool-ok"),
        "Process output must contain expected stdout banner: {stdout}"
    );
}

#[tokio::test]
async fn test_real_sandbox_http_404_error_cleans_up_temporary_files() {
    let sandbox = temporary_sandbox();
    let dest_path = sandbox.path().join("sandbox_tool_404");

    let (url, server_handle) = serve_sandbox_binary("404 Not Found", b"not found".to_vec());

    let client = HttpToolDownloaderClient::default();
    let result = client.download_binary(&url, &dest_path).await;
    server_handle
        .join()
        .expect("server thread terminates cleanly");

    assert!(result.is_err(), "Download must return error on 404");
    let err_msg = result.unwrap_err().message;
    assert!(
        err_msg.contains("404"),
        "Error message should mention 404: {err_msg}"
    );

    assert!(
        !dest_path.exists(),
        "Target file must not be created on 404"
    );
    let temp_dest = dest_path.with_extension("download.tmp");
    assert!(
        !temp_dest.exists(),
        "Temp download file must not linger after 404"
    );
}

#[tokio::test]
async fn test_real_sandbox_http_500_error_cleans_up_temporary_files() {
    let sandbox = temporary_sandbox();
    let dest_path = sandbox.path().join("sandbox_tool_500");

    let (url, server_handle) =
        serve_sandbox_binary("500 Internal Server Error", b"server error".to_vec());

    let client = HttpToolDownloaderClient::default();
    let result = client.download_binary(&url, &dest_path).await;
    server_handle
        .join()
        .expect("server thread terminates cleanly");

    assert!(result.is_err(), "Download must return error on 500");
    let err_msg = result.unwrap_err().message;
    assert!(
        err_msg.contains("500"),
        "Error message should mention 500: {err_msg}"
    );

    assert!(
        !dest_path.exists(),
        "Target file must not be created on 500"
    );
    let temp_dest = dest_path.with_extension("download.tmp");
    assert!(
        !temp_dest.exists(),
        "Temp download file must not linger after 500"
    );
}

#[tokio::test]
async fn test_real_sandbox_truncated_stream_cleans_up_temporary_files() {
    let sandbox = temporary_sandbox();
    let dest_path = sandbox.path().join("sandbox_tool_truncated");

    // Header promises 5000 bytes, but server only writes 50 bytes and closes connection
    let (url, server_handle) = serve_truncated_binary(5000, vec![b'X'; 50]);

    let client = HttpToolDownloaderClient::default();
    let result = client.download_binary(&url, &dest_path).await;
    server_handle
        .join()
        .expect("server thread terminates cleanly");

    assert!(result.is_err(), "Download must fail on truncated stream");
    assert!(
        !dest_path.exists(),
        "Target file must not exist after truncated stream"
    );
    let temp_dest = dest_path.with_extension("download.tmp");
    assert!(
        !temp_dest.exists(),
        "Temp file must be cleaned up after truncated stream"
    );
}

#[tokio::test]
async fn test_real_end_to_end_media_tools_manager_sandbox_with_mirror_override() {
    let sandbox = temporary_sandbox();
    let managed_dir = sandbox.path().join("managed_tools");

    let script = sample_executable_script().to_vec();
    let (url, server_handle) = serve_sandbox_binary("200 OK", script);

    // Set production mirror override environment variable pointing to the real sandbox server
    std::env::set_var("AD_FFMPEG_MIRROR", &url);

    let resolver = ToolPathResolver::with_managed_dir(managed_dir.clone());
    let client = Arc::new(HttpToolDownloaderClient::default());
    let manager = MediaToolsManager::new(resolver, client);

    // 1. Initial status: tool is missing from the managed sandbox
    let status_before = manager.status(MediaToolType::Ffmpeg);
    // Note: status_before might be available if system has ffmpeg in PATH, but source is not ManagedUserData
    if status_before.is_available {
        assert_ne!(
            status_before.source,
            Some(ToolResolutionSource::ManagedUserData)
        );
    }

    // 2. Real download and installation through MediaToolsManager
    let installed_path = manager
        .install_managed_tool(MediaToolType::Ffmpeg)
        .await
        .expect("Real installation into sandbox should succeed");

    server_handle
        .join()
        .expect("server thread terminates cleanly");
    assert!(installed_path.exists(), "Installed binary must exist");
    assert!(
        installed_path.starts_with(&managed_dir),
        "Installed binary must be inside managed sandbox dir"
    );

    // 3. Post-install status: tool must resolve to ManagedUserData
    let status_after = manager.status(MediaToolType::Ffmpeg);
    assert!(
        status_after.is_available,
        "Tool must be available after real install"
    );
    assert_eq!(
        status_after.source,
        Some(ToolResolutionSource::ManagedUserData),
        "Tool resolution source must be ManagedUserData"
    );
    assert_eq!(status_after.path, Some(installed_path.clone()));

    // 4. Real execution of installed binary from the sandbox
    let output = Command::new(&installed_path)
        .output()
        .expect("Execute installed sandbox binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sandbox-media-tool-ok"));

    std::env::remove_var("AD_FFMPEG_MIRROR");
}

#[tokio::test]
async fn test_real_sandbox_tiered_resolution_and_precedence() {
    let sandbox = temporary_sandbox();

    let env_tools_dir = sandbox.path().join("env_tools");
    let managed_tools_dir = sandbox.path().join("managed_tools");
    std::fs::create_dir_all(&env_tools_dir).unwrap();
    std::fs::create_dir_all(&managed_tools_dir).unwrap();

    let exe_name = host_executable_name(MediaToolType::YtDlp);
    let env_bin = env_tools_dir.join(exe_name);
    let managed_bin = managed_tools_dir.join(exe_name);

    #[cfg(not(windows))]
    {
        std::fs::write(&env_bin, b"#!/bin/sh\necho \"tier1-env\"\nexit 0\n").unwrap();
        std::fs::write(&managed_bin, b"#!/bin/sh\necho \"tier2-managed\"\nexit 0\n").unwrap();
        std::fs::set_permissions(&env_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&managed_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        std::fs::write(&env_bin, b"@echo tier1-env\r\n").unwrap();
        std::fs::write(&managed_bin, b"@echo tier2-managed\r\n").unwrap();
    }

    // Step 1: Set Tier 1 environment variable override
    std::env::set_var("AD_YTDLP_PATH", env_bin.to_str().unwrap());
    let resolver = ToolPathResolver::with_managed_dir(managed_tools_dir.clone());

    let (resolved_path, source) = resolver
        .resolve_tool(MediaToolType::YtDlp)
        .expect("Tier 1 resolve");
    assert_eq!(source, ToolResolutionSource::EnvironmentOverride);
    assert_eq!(resolved_path, env_bin);

    let output = Command::new(&resolved_path).output().expect("run tier 1");
    assert!(String::from_utf8_lossy(&output.stdout).contains("tier1-env"));

    // Step 2: Remove Tier 1 override -> Tier 2 (ManagedUserData) must take over
    std::env::remove_var("AD_YTDLP_PATH");
    let (resolved_path2, source2) = resolver
        .resolve_tool(MediaToolType::YtDlp)
        .expect("Tier 2 resolve");
    assert_eq!(source2, ToolResolutionSource::ManagedUserData);
    assert_eq!(resolved_path2, managed_bin);

    let output2 = Command::new(&resolved_path2).output().expect("run tier 2");
    assert!(String::from_utf8_lossy(&output2.stdout).contains("tier2-managed"));
}

#[test]
fn test_real_host_ffmpeg_execution_if_present() {
    if let Some(ffmpeg_path) = find_tool_path("ffmpeg") {
        assert!(
            ffmpeg_path.is_file(),
            "Resolved path must be an existing file"
        );
        let output = Command::new(&ffmpeg_path)
            .arg("-version")
            .output()
            .expect("Execute host ffmpeg -version");
        assert!(output.status.success(), "ffmpeg -version should succeed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("ffmpeg version"),
            "Output must contain 'ffmpeg version': {stdout}"
        );
    }
}

#[test]
fn test_download_specs_select_native_windows_arm64_binaries() {
    let ytdlp_arm64 = download_spec_for(MediaToolType::YtDlp, ToolPlatform::WindowsArm64)
        .expect("ARM64 yt-dlp spec");
    assert!(ytdlp_arm64.url.ends_with("/yt-dlp_arm64.exe"));
    assert_eq!(ytdlp_arm64.binary_name, "yt-dlp.exe");

    let ytdlp_x64 =
        download_spec_for(MediaToolType::YtDlp, ToolPlatform::WindowsX64).expect("x64 yt-dlp spec");
    assert!(ytdlp_x64.url.ends_with("/yt-dlp.exe"));
    assert!(!ytdlp_x64.url.contains("arm64"));

    let ffmpeg_arm64 = download_spec_for(MediaToolType::Ffmpeg, ToolPlatform::WindowsArm64)
        .expect("ARM64 ffmpeg spec");
    assert!(ffmpeg_arm64.url.contains("winarm64"));
    assert!(ffmpeg_arm64.url.ends_with(".zip"));
    assert_eq!(ffmpeg_arm64.binary_name, "ffmpeg.exe");

    let ffmpeg_x64 = download_spec_for(MediaToolType::Ffmpeg, ToolPlatform::WindowsX64)
        .expect("x64 ffmpeg spec");
    assert!(!ffmpeg_x64.url.contains("winarm64"));
    assert_eq!(ffmpeg_x64.binary_name, "ffmpeg.exe");
}

#[test]
fn test_download_specs_cover_other_arm64_platforms_and_fail_loudly() {
    let ytdlp_linux_arm64 = download_spec_for(MediaToolType::YtDlp, ToolPlatform::LinuxArm64)
        .expect("Linux ARM64 yt-dlp spec");
    assert!(ytdlp_linux_arm64.url.ends_with("/yt-dlp_linux_aarch64"));
    assert_eq!(ytdlp_linux_arm64.binary_name, "yt-dlp");

    let ytdlp_macos_arm64 = download_spec_for(MediaToolType::YtDlp, ToolPlatform::MacosArm64)
        .expect("macOS ARM64 yt-dlp spec");
    assert!(ytdlp_macos_arm64.url.ends_with("/yt-dlp_macos"));

    let ffmpeg_linux_arm64 = download_spec_for(MediaToolType::Ffmpeg, ToolPlatform::LinuxArm64)
        .expect_err("Linux ARM64 ffmpeg must not silently use an x64 binary");
    assert!(
        ffmpeg_linux_arm64.message.contains("Linux ARM64"),
        "Error must explain the unsupported platform: {}",
        ffmpeg_linux_arm64.message
    );
}

#[test]
fn test_platform_adapters_expose_native_executable_names() {
    let windows = WindowsAdapter::arm64();
    assert_eq!(windows.id(), "windows-arm64");
    assert_eq!(windows.executable_name(MediaToolType::Ffmpeg), "ffmpeg.exe");
    assert_eq!(windows.executable_name(MediaToolType::YtDlp), "yt-dlp.exe");

    let macos = MacosAdapter::arm64();
    assert_eq!(macos.id(), "macos-arm64");
    assert_eq!(macos.executable_name(MediaToolType::Ffmpeg), "ffmpeg");
    assert_eq!(macos.executable_name(MediaToolType::YtDlp), "yt-dlp");

    let linux = LinuxAdapter::x64();
    assert_eq!(linux.id(), "linux-x64");
    assert_eq!(linux.executable_name(MediaToolType::Ffmpeg), "ffmpeg");
    assert_eq!(linux.executable_name(MediaToolType::YtDlp), "yt-dlp");

    let unsupported = UnsupportedAdapter;
    assert!(
        unsupported.download_spec(MediaToolType::Ffmpeg).is_err(),
        "Unsupported platforms must fail loudly instead of guessing a binary"
    );
}

#[test]
fn test_platform_adapters_expose_bundled_sidecar_locations() {
    let macos_app_dir = PathBuf::from("/Applications/Open Download Manager.app/Contents/MacOS");
    let macos_candidates = MacosAdapter::arm64().bundled_tool_candidates("ffmpeg", &macos_app_dir);
    assert!(macos_candidates.contains(&PathBuf::from(
        "/Applications/Open Download Manager.app/Contents/Resources/tools/ffmpeg"
    )));

    let linux_app_dir = PathBuf::from("/opt/open-download-manager");
    let linux_candidates = LinuxAdapter::arm64().bundled_tool_candidates("ffmpeg", &linux_app_dir);
    assert!(linux_candidates.contains(&PathBuf::from(
        "/usr/lib/open-download-manager/tools/ffmpeg"
    )));
    assert!(linux_candidates.contains(&PathBuf::from(
        "/usr/libexec/open-download-manager/tools/ffmpeg"
    )));

    let windows_app_dir = PathBuf::from(r"C:\Program Files\Open Download Manager");
    let windows_candidates =
        WindowsAdapter::x64().bundled_tool_candidates("ffmpeg.exe", &windows_app_dir);
    assert_eq!(
        windows_candidates,
        vec![
            windows_app_dir.join("tools").join("ffmpeg.exe"),
            windows_app_dir.join("ffmpeg.exe"),
        ]
    );
}

#[test]
fn test_resolver_uses_adapter_executable_names() {
    let sandbox = temporary_sandbox();
    let managed_dir = sandbox.path().join("managed_tools");
    std::fs::create_dir_all(&managed_dir).expect("managed tools directory");

    // Use ffmpeg so this test does not race the yt-dlp environment override test.
    std::env::remove_var("AD_FFMPEG_PATH");

    let windows_exe = managed_dir.join("ffmpeg.exe");
    std::fs::write(&windows_exe, b"windows-arm64-placeholder").expect("write managed tool");

    let resolver = ToolPathResolver::with_adapter_and_managed_dir(
        Arc::new(WindowsAdapter::arm64()),
        managed_dir.clone(),
    );
    let (resolved, source) = resolver
        .resolve_tool(MediaToolType::Ffmpeg)
        .expect("Windows adapter must resolve .exe names");

    assert_eq!(source, ToolResolutionSource::ManagedUserData);
    assert_eq!(resolved, windows_exe);
}

fn build_zip_with_entry(entry_name: &str, contents: &[u8]) -> Vec<u8> {
    use std::io::Cursor;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file(entry_name, options)
            .expect("start zip entry");
        writer.write_all(contents).expect("write zip entry");
        writer.finish().expect("finish zip archive");
    }
    buffer.into_inner()
}

#[tokio::test]
async fn test_zip_archive_install_extracts_native_executable_without_external_unzip() {
    let sandbox = temporary_sandbox();
    let managed_dir = sandbox.path().join("managed_tools");
    let exe_name = host_executable_name(MediaToolType::Ffmpeg);
    let binary_contents = b"native-media-tool-payload";

    let archive = build_zip_with_entry(&format!("ffmpeg-build/bin/{exe_name}"), binary_contents);
    let (base_url, server_handle) = serve_sandbox_binary("200 OK", archive);
    let archive_url = format!("{base_url}.zip");

    let resolver = ToolPathResolver::with_managed_dir(managed_dir.clone());
    let manager = MediaToolsManager::new(resolver, Arc::new(HttpToolDownloaderClient::default()));

    let installed_path = manager
        .install_managed_tool_from_url(MediaToolType::Ffmpeg, &archive_url)
        .await
        .expect("zip installation must use the in-process extractor");

    server_handle
        .join()
        .expect("server thread terminates cleanly");

    assert_eq!(installed_path, managed_dir.join(exe_name));
    assert_eq!(
        std::fs::read(&installed_path).expect("read installed binary"),
        binary_contents
    );
    assert!(
        !managed_dir.join(format!("{exe_name}.archive.tmp")).exists(),
        "Temporary archive must be removed after extraction"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&installed_path)
            .expect("installed metadata")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "Extracted binary must be executable");
    }
}

#[tokio::test]
async fn test_zip_archive_install_rejects_path_traversal() {
    let sandbox = temporary_sandbox();
    let managed_dir = sandbox.path().join("managed_tools");
    let exe_name = host_executable_name(MediaToolType::Ffmpeg);

    let archive = build_zip_with_entry(&format!("../{exe_name}"), b"malicious");
    let (base_url, server_handle) = serve_sandbox_binary("200 OK", archive);
    let archive_url = format!("{base_url}.zip");

    let resolver = ToolPathResolver::with_managed_dir(managed_dir.clone());
    let manager = MediaToolsManager::new(resolver, Arc::new(HttpToolDownloaderClient::default()));

    let result = manager
        .install_managed_tool_from_url(MediaToolType::Ffmpeg, &archive_url)
        .await;
    server_handle
        .join()
        .expect("server thread terminates cleanly");

    assert!(result.is_err(), "Path traversal entry must be rejected");
    assert!(
        !managed_dir.join(exe_name).exists(),
        "Rejected archive must not install a binary"
    );
    assert!(
        !sandbox.path().join(exe_name).exists(),
        "Rejected archive must not escape the managed tools directory"
    );
}
