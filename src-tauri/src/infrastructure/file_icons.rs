use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::domain::{category::DownloadCategory, download::DownloadRecord, file_type::FileType};

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
    let clean_ext = extension.trim_start_matches('.');
    if let Some(file_type) = FileType::from_extension(clean_ext) {
        return file_type.svg_data_url();
    }
    match DownloadCategory::from_extension(clean_ext) {
        DownloadCategory::Video => FileType::Video.svg_data_url(),
        DownloadCategory::Audio => FileType::Audio.svg_data_url(),
        DownloadCategory::Archives => FileType::Zip.svg_data_url(),
        DownloadCategory::Documents => FileType::Text.svg_data_url(),
        DownloadCategory::Programs => FileType::Exe.svg_data_url(),
        DownloadCategory::Other => FileType::default_svg_data_url(),
    }
}
