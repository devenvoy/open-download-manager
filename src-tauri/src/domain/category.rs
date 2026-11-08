// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: integrate standard crates (mime_guess, sanitize-filename, tempfile, which) and FileType vector icon catalog
// Description: Download classification categories and file extension / MIME type classification heuristics.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum DownloadCategory {
    Video,
    Audio,
    Archives,
    Documents,
    Programs,
    Other,
}

impl DownloadCategory {
    pub const ALL: &[Self] = &[
        Self::Video,
        Self::Audio,
        Self::Archives,
        Self::Documents,
        Self::Programs,
        Self::Other,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Archives => "Archives",
            Self::Documents => "Documents",
            Self::Programs => "Programs",
            Self::Other => "Other",
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        if let Some(file_type) = crate::domain::file_type::FileType::from_extension(ext) {
            return file_type.category();
        }
        let guess = mime_guess::from_ext(ext.trim_start_matches('.')).first();
        if let Some(mime) = guess {
            return Self::from_mime(&mime);
        }
        Self::Other
    }

    pub fn from_path(path: &std::path::Path) -> Self {
        if let Some(file_type) = crate::domain::file_type::FileType::from_path(path) {
            return file_type.category();
        }
        let guess = mime_guess::from_path(path).first();
        if let Some(mime) = guess {
            return Self::from_mime(&mime);
        }
        Self::Other
    }

    pub fn from_mime(mime: &mime_guess::mime::Mime) -> Self {
        use mime_guess::mime;
        match mime.type_() {
            mime::VIDEO => Self::Video,
            mime::AUDIO => Self::Audio,
            mime::TEXT => Self::Documents,
            mime::APPLICATION => {
                let sub = mime.subtype().as_str();
                match sub {
                    "pdf" | "msword" | "rtf" | "epub+zip" => Self::Documents,
                    s if s.contains("officedocument") || s.contains("opendocument") => {
                        Self::Documents
                    }
                    "zip" | "x-7z-compressed" | "x-rar-compressed" | "x-tar" | "gzip" | "x-bzip2"
                    | "x-xz" | "x-iso9660-image" => Self::Archives,
                    "x-msdownload"
                    | "x-executable"
                    | "vnd.microsoft.portable-executable"
                    | "x-apple-diskimage"
                    | "vnd.android.package-archive" => Self::Programs,
                    _ => Self::Other,
                }
            }
            _ => Self::Other,
        }
    }
}

impl fmt::Display for DownloadCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
