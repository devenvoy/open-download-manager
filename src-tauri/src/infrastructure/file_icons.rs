use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::Engine;
use image::{Rgba, RgbaImage};

use crate::domain::download::DownloadRecord;

pub fn is_safe_extension(ext: &str) -> bool {
    if !ext.starts_with('.') {
        return false;
    }
    let chars = &ext[1..];
    if chars.is_empty() || chars.len() > 12 {
        return false;
    }
    chars.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn extension_key(download_source: &str) -> String {
    let normalized = download_source.replace('\\', "/");
    let path = Path::new(&normalized);
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let candidate = format!(".{}", ext.to_ascii_lowercase());
        if is_safe_extension(&candidate) {
            return candidate;
        }
    }
    String::new()
}

pub fn extension_key_from_download(download: &DownloadRecord) -> String {
    if !download.filename.is_empty() {
        let key = extension_key(&download.filename);
        if !key.is_empty() {
            return key;
        }
    }
    if let Some(ref path) = download.path {
        let key = extension_key(path);
        if !key.is_empty() {
            return key;
        }
    }
    String::new()
}

#[derive(Clone)]
pub struct FileIconProvider {
    cache_directory: PathBuf,
    icons: Arc<Mutex<HashMap<String, String>>>,
}

impl FileIconProvider {
    pub fn new(cache_directory: PathBuf) -> Self {
        Self {
            cache_directory,
            icons: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn load_extension(&self, extension: &str) -> bool {
        if extension.is_empty() {
            return false;
        }

        {
            let icons = self.icons.lock().unwrap();
            if icons.contains_key(extension) {
                return false;
            }
        }

        // Write probe file to cache directory: associated-file{extension}
        let _ = std::fs::create_dir_all(&self.cache_directory);
        let probe_path = self
            .cache_directory
            .join(format!("associated-file{extension}"));
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&probe_path);

        let data_url = generate_icon_data_url(extension);

        let mut icons = self.icons.lock().unwrap();
        icons.insert(extension.to_owned(), data_url);
        true
    }

    pub fn load(&self, downloads: &[DownloadRecord]) -> bool {
        let mut extensions = Vec::new();
        for download in downloads {
            let key = extension_key_from_download(download);
            if !key.is_empty() && !extensions.contains(&key) {
                extensions.push(key);
            }
        }

        let mut newly_loaded = false;
        for ext in extensions {
            if self.load_extension(&ext) {
                newly_loaded = true;
            }
        }
        newly_loaded
    }

    pub fn snapshot(&self) -> BTreeMap<String, String> {
        let icons = self.icons.lock().unwrap();
        icons
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

pub fn generate_icon_data_url(extension: &str) -> String {
    let mut img = RgbaImage::new(16, 16);
    let color = match extension {
        ".mp4" | ".mkv" | ".avi" | ".mov" | ".webm" | ".flv" => Rgba([99, 102, 241, 255]),
        ".mp3" | ".wav" | ".flac" | ".aac" | ".ogg" | ".m4a" => Rgba([245, 158, 11, 255]),
        ".png" | ".jpg" | ".jpeg" | ".gif" | ".webp" | ".svg" => Rgba([16, 185, 129, 255]),
        ".zip" | ".rar" | ".7z" | ".tar" | ".gz" | ".bz2" => Rgba([234, 179, 8, 255]),
        ".pdf" => Rgba([239, 68, 68, 255]),
        ".exe" | ".msi" | ".dmg" | ".app" | ".deb" | ".rpm" => Rgba([139, 92, 246, 255]),
        _ => Rgba([156, 163, 175, 255]),
    };

    for y in 1..15 {
        for x in 2..14 {
            if x >= 10 && y <= 4 && (x - 10 + y) > 4 {
                continue;
            }
            if x == 2 || x == 13 || y == 1 || y == 14 || (x >= 10 && y <= 4 && (x - 10 + y) == 4) {
                img.put_pixel(x, y, color);
            } else {
                img.put_pixel(x, y, Rgba([color[0], color[1], color[2], 40]));
            }
        }
    }

    let mut buffer = Cursor::new(Vec::new());
    if img.write_to(&mut buffer, image::ImageFormat::Png).is_ok() {
        let base64_str = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());
        format!("data:image/png;base64,{base64_str}")
    } else {
        String::new()
    }
}
