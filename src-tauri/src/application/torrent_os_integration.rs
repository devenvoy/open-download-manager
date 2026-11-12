// Created: 2026-11-11
// Reason: feat(torrent): register ODM as default magnet/.torrent handler
// Description: OS-level registration so clicking a magnet link or .torrent file
// launches Open Download Manager. Also processes launch arguments when the OS
// opens ODM via protocol or file association.

use std::path::PathBuf;
#[cfg(feature = "torrent")]
use std::sync::Mutex;
#[cfg(feature = "torrent")]
use std::time::{Duration, Instant};

#[cfg(all(feature = "torrent", target_os = "windows"))]
use std::process::Command;

use tauri::AppHandle;
#[cfg(feature = "torrent")]
use tauri::{Emitter, Manager};

#[cfg(feature = "torrent")]
use crate::{
    domain::torrent::TorrentReviewRequest,
    presentation::app_state::AppState,
};

pub const MAGNET_SCHEME: &str = "magnet";
pub const TORRENT_EXTENSION: &str = "torrent";

/// How long a magnet link or .torrent path stays "already handled".
#[cfg(feature = "torrent")]
const RECENT_HANDLE_WINDOW: Duration = Duration::from_secs(60);

/// Remembers the magnet links and .torrent paths that were handled recently.
///
/// The same request can reach ODM through several channels at once: the
/// launch arguments of a cold start, the single-instance callback of a warm
/// start, and the deep-link event the plugin derives from `RunEvent::Opened`.
/// Without this guard the torrent would be added more than once.
#[cfg(feature = "torrent")]
static RECENT_HANDLES: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

/// Records the handle and reports whether this is the first time it is seen.
#[cfg(feature = "torrent")]
pub fn claim_handle(key: &str) -> bool {
    let Ok(mut recent) = RECENT_HANDLES.lock() else {
        return true;
    };
    let now = Instant::now();
    recent.retain(|(_, at)| now.duration_since(*at) < RECENT_HANDLE_WINDOW);
    if recent.iter().any(|(seen, _)| seen == key) {
        return false;
    }
    recent.push((key.to_owned(), now));
    true
}

/// True when the argument looks like a magnet link or a .torrent file path.
pub fn is_torrent_launch_arg(arg: &str) -> bool {
    let trimmed = arg.trim();
    if trimmed.starts_with("magnet:") {
        return true;
    }
    if trimmed.contains("://") {
        // A remote URL that happens to end in .torrent is a download link, not
        // a local file the OS handed to ODM.
        return false;
    }
    PathBuf::from(trimmed)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case(TORRENT_EXTENSION))
        .unwrap_or(false)
}

/// Extracts magnet links and .torrent paths from launch arguments.
pub fn extract_torrent_launch_args(args: &[String]) -> Vec<String> {
    args.iter()
        .skip(1) // skip the executable name
        .filter(|arg| is_torrent_launch_arg(arg))
        .cloned()
        .collect()
}

/// Registers ODM as the default handler for magnet links and .torrent files.
/// On Windows this writes to HKCU (no admin rights required).
/// On macOS this asks LaunchServices to make ODM the default application for
/// the `magnet:` scheme and for the `.torrent` content type. The bundle
/// declarations in tauri.conf.json only make ODM a *candidate*: other torrent
/// clients register `magnet:` with `LSHandlerRank = Owner`, so without an
/// explicit claim the OS keeps routing magnet links and .torrent files to
/// whichever app claimed them last.
/// On Linux it is handled by the desktop MimeType entry.
pub fn register_torrent_associations() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        register_windows_associations()?;
    }
    #[cfg(all(feature = "torrent", target_os = "macos"))]
    {
        register_macos_associations()?;
    }
    Ok(())
}

#[cfg(all(feature = "torrent", target_os = "windows"))]
fn register_windows_associations() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Could not get executable path: {e}"))?;
    let exe_str = exe_path.to_string_lossy();

    // Register magnet URL protocol.
    let add = |key: &str, values: &[(&str, &str)]| -> Result<(), String> {
        for (name, value) in values {
            let status = Command::new("reg")
                .args([
                    "add",
                    key,
                    "/v",
                    name,
                    "/d",
                    value,
                    "/f",
                ])
                .status()
                .map_err(|e| format!("Could not run reg.exe: {e}"))?;
            if !status.success() {
                return Err(format!("reg add {key} {name} failed"));
            }
        }
        Ok(())
    };

    let command_value = format!("\"{}\" \"%1\"", exe_str);

    add(
        r"HKCU\Software\Classes\magnet",
        &[("", "URL:Magnet Protocol"), ("URL Protocol", "")],
    )?;
    add(
        r"HKCU\Software\Classes\magnet\shell\open\command",
        &[("", &command_value)],
    )?;

    add(
        r"HKCU\Software\Classes\.torrent",
        &[("", "OpenDownloadManager.Torrent")],
    )?;
    add(
        r"HKCU\Software\Classes\OpenDownloadManager.Torrent",
        &[("", "BitTorrent File")],
    )?;
    add(
        r"HKCU\Software\Classes\OpenDownloadManager.Torrent\shell\open\command",
        &[("", &command_value)],
    )?;

    Ok(())
}

#[cfg(all(feature = "torrent", not(target_os = "windows")))]
#[allow(dead_code)]
fn register_windows_associations() -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "torrent"))]
#[allow(dead_code)]
fn register_windows_associations() -> Result<(), String> {
    Ok(())
}

/// Asks LaunchServices to make ODM the default application for the `magnet:`
/// scheme and for the `.torrent` content type.
///
/// The bundle declarations (`CFBundleURLTypes`, `CFBundleDocumentTypes`) only
/// make ODM a candidate handler. Other torrent clients register `magnet:`
/// with `LSHandlerRank = Owner`, and macOS remembers a per-user default in
/// `LSHandlers`, so the OS keeps routing magnet links and `.torrent` files to
/// whichever app claimed them last. Only an explicit request to
/// LaunchServices changes that.
#[cfg(all(feature = "torrent", target_os = "macos"))]
fn register_macos_associations() -> Result<(), String> {
    use block2::RcBlock;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSError, NSString, NSURL};

    // <bundle>.app/Contents/MacOS/<binary> -> <bundle>.app
    let exe = std::env::current_exe()
        .map_err(|error| format!("Could not get executable path: {error}"))?;
    let mut bundle_path = exe.as_path();
    for _ in 0..3 {
        bundle_path = match bundle_path.parent() {
            Some(parent) => parent,
            None => return Ok(()),
        };
    }
    let bundle_path = bundle_path.to_string_lossy().to_string();
    if !bundle_path.ends_with(".app") {
        // Development build: there is no application bundle to register.
        return Ok(());
    }
    let bundle_url = NSURL::fileURLWithPath(&NSString::from_str(&bundle_path));

    let workspace = NSWorkspace::sharedWorkspace();

    // 1. magnet: links.
    let scheme = NSString::from_str(MAGNET_SCHEME);
    let magnet_handler = RcBlock::new(|_error: *mut NSError| {});
    workspace.setDefaultApplicationAtURL_toOpenURLsWithScheme_completionHandler(
        &bundle_url,
        &scheme,
        Some(&magnet_handler),
    );

    // 2. .torrent files. LaunchServices derives the content type from the file
    //    extension, so a probe file with the right extension resolves to the
    //    same content type as any real .torrent file.
    let probe = std::env::temp_dir().join(format!("odm-{}.torrent", std::process::id()));
    if std::fs::write(&probe, b"").is_err() {
        return Ok(());
    }
    let probe_path = probe.to_string_lossy().to_string();
    let file_url = NSURL::fileURLWithPath(&NSString::from_str(&probe_path));
    let cleanup_path = probe.clone();
    let torrent_handler = RcBlock::new(move |_error: *mut NSError| {
        let _ = std::fs::remove_file(&cleanup_path);
    });
    workspace.setDefaultApplicationAtURL_toOpenContentTypeOfFileAtURL_completionHandler(
        &bundle_url,
        &file_url,
        Some(&torrent_handler),
    );

    Ok(())
}

/// Hands a review to the Torrents window.
///
/// The pending copy is stored *before* the event goes out. Otherwise the
/// window could pick the event up, ask for the pending copy, and find nothing
/// there yet - the backend would store it a moment later and the window's
/// poll would hand out the very same review a second time, which made the
/// confirmation panel reappear after the user had already answered it.
///
/// The window drops the pending copy as soon as it receives the event, so a
/// review is only ever handed out once. The copy stays only when no window was
/// listening yet, so a window that opens later still finds it.
#[cfg(feature = "torrent")]
pub fn deliver_review(
    app: &AppHandle,
    torrents: &crate::application::torrent_service::TorrentService,
    review: TorrentReviewRequest,
) {
    torrents.set_pending_review(Some(review.clone()));
    let _ = app.emit("torrent://review", &review);
}

/// Handles magnet links and .torrent files passed as launch arguments.
/// Adds them in review mode (metadata fetched, no payload yet) and opens the
/// torrent window so the user can pick files and confirm the download.
#[cfg(feature = "torrent")]
pub fn handle_torrent_launch_args(app: &AppHandle, args: &[String]) -> Result<(), String> {
    let torrent_args = extract_torrent_launch_args(args);
    if torrent_args.is_empty() {
        return Ok(());
    }

    for arg in torrent_args {
        if arg.starts_with("magnet:") {
            let _ = handle_torrent_url(app, &arg);
        } else {
            let _ = handle_torrent_file(app, &arg);
        }
    }

    Ok(())
}

/// Handles a single magnet URL received via deep-link (macOS/Windows).
#[cfg(feature = "torrent")]
pub fn handle_torrent_url(app: &AppHandle, url: &str) -> Result<(), String> {
    if !url.starts_with("magnet:") {
        return Ok(());
    }
    if !claim_handle(url) {
        // The same magnet link already reached ODM through another channel.
        return Ok(());
    }
    let app = app.clone();
    let url = url.to_string();
    // Open the Torrents window first so the user sees the review UI while the
    // engine resolves metadata (which can take several seconds for a magnet).
    let _ = crate::presentation::window_manager::open_torrent_window(app.clone());
    tauri::async_runtime::spawn(async move {
        let _ = handle_torrent_url_internal(&app, &url).await;
    });
    Ok(())
}

/// Handles a single .torrent file path received via file open (macOS).
#[cfg(feature = "torrent")]
pub fn handle_torrent_file(app: &AppHandle, path: &str) -> Result<(), String> {
    let Some(path) = torrent_file_path(path) else {
        return Ok(());
    };
    if !path.to_string_lossy().ends_with(".torrent")
        && !path.to_string_lossy().ends_with(".TORRENT")
    {
        return Ok(());
    }
    let key = path.to_string_lossy().to_string();
    if !claim_handle(&key) {
        // The same file already reached ODM through another channel.
        return Ok(());
    }
    let app = app.clone();
    let path = key;
    // Open the Torrents window first so the user sees the review UI immediately.
    let _ = crate::presentation::window_manager::open_torrent_window(app.clone());
    tauri::async_runtime::spawn(async move {
        let _ = handle_torrent_file_internal(&app, &path).await;
    });
    Ok(())
}

/// Turns a `.torrent` reference into a local file path.
///
/// macOS reports opened documents as `file://` URLs, while Windows and Linux
/// pass the plain path.
#[cfg(feature = "torrent")]
fn torrent_file_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.starts_with("file:") {
        return url::Url::parse(trimmed)
            .ok()
            .and_then(|parsed| parsed.to_file_path().ok());
    }
    Some(PathBuf::from(trimmed))
}

/// Internal: adds a magnet URL in review mode and emits the review event.
#[cfg(feature = "torrent")]
async fn handle_torrent_url_internal(app: &AppHandle<tauri::Wry>, url: &str) -> Result<(), String> {
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => {
            eprintln!("[odm] AppState not available for torrent URL");
            return Ok(());
        }
    };
    let torrents = state.torrents();

    let options = crate::domain::torrent::TorrentAddOptions {
        review: true,
        ..Default::default()
    };

    // The engine boots asynchronously, so a cold-start deep link can arrive
    // before it is ready. `add_magnet_when_ready` waits for it.
    match torrents
        .add_magnet_when_ready(url.to_string(), options)
        .await
    {
        Ok(added) => {
            let review = TorrentReviewRequest {
                id: added.id.unwrap_or(0),
                name: added.name.clone(),
                info_hash: added.info_hash.clone(),
            };
            let name = review.name.clone();
            deliver_review(app, &torrents, review);
            eprintln!("[odm] Deep-link torrent review: name={name}");
        }
        Err(error) => {
            eprintln!(
                "[odm] Failed to add magnet from deep-link: {}",
                error.message
            );
        }
    }
    Ok(())
}

/// Internal: reads a .torrent file and adds it in review mode, emitting the review event.
#[cfg(feature = "torrent")]
async fn handle_torrent_file_internal(app: &AppHandle<tauri::Wry>, path: &str) -> Result<(), String> {
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => {
            eprintln!("[odm] AppState not available for torrent file");
            return Ok(());
        }
    };
    let torrents = state.torrents();

    let options = crate::domain::torrent::TorrentAddOptions {
        review: true,
        ..Default::default()
    };

    match tokio::fs::read(path).await {
        Ok(bytes) => {
            match torrents.add_file_bytes(bytes, options).await {
                Ok(added) => {
                    let review = TorrentReviewRequest {
                        id: added.id.unwrap_or(0),
                        name: added.name.clone(),
                        info_hash: added.info_hash.clone(),
                    };
                    let name = review.name.clone();
                    deliver_review(app, &torrents, review);
                    eprintln!("[odm] File-open torrent review: name={name}");
                }
                Err(e) => {
                    eprintln!(
                        "[odm] Failed to add .torrent from file open: {}",
                        e.message
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("[odm] Could not read .torrent file {path}: {e}");
        }
    }
    Ok(())
}

#[cfg(not(feature = "torrent"))]
pub fn deliver_review(
    _app: &AppHandle,
    _torrents: &crate::application::torrent_service::TorrentService,
    _review: crate::domain::torrent::TorrentReviewRequest,
) {
}

#[cfg(not(feature = "torrent"))]
pub fn handle_torrent_launch_args(_app: &AppHandle, _args: &[String]) -> Result<(), String> {
    Ok(())
}
#[cfg(not(feature = "torrent"))]
pub fn handle_torrent_url(_app: &AppHandle, _url: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "torrent"))]
pub fn handle_torrent_file(_app: &AppHandle, _path: &str) -> Result<(), String> {
    Ok(())
}
