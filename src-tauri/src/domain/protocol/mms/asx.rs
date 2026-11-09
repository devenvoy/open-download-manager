// Created: 2026-11-10
// Reason: refactor: modularize MMS domain protocol into variant, session, and asx parser submodules
// Description: Safe, bounded XML and INI ASX/WVX playlist parser.

use serde::{Deserialize, Serialize};

/// Maximum allowed playlist file size in bytes to prevent memory blowup (1 MiB).
pub const MAX_ASX_PLAYLIST_BYTES: usize = 1_048_576;

/// Maximum number of media entries parsed from an ASX/WVX playlist.
pub const MAX_ASX_ENTRIES: usize = 500;

/// A media entry parsed from an ASX/WVX playlist wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsxEntry {
    pub title: Option<String>,
    pub href: String,
    pub scheme: Option<String>,
    pub is_supported_fallback: bool,
    pub is_mms: bool,
}

/// Bounded playlist representation parsed from an ASX or WVX file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsxPlaylist {
    pub title: Option<String>,
    pub entries: Vec<AsxEntry>,
}

impl AsxPlaylist {
    /// Extracts all HTTP and HTTPS fallback URLs from the playlist.
    pub fn supported_fallback_urls(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.is_supported_fallback)
            .map(|e| e.href.clone())
            .collect()
    }

    /// Extracts all MMS stream URLs from the playlist.
    pub fn mms_urls(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.is_mms)
            .map(|e| e.href.clone())
            .collect()
    }

    /// Whether the playlist contains at least one downloadable HTTP/HTTPS fallback stream.
    pub fn has_supported_fallback(&self) -> bool {
        self.entries.iter().any(|e| e.is_supported_fallback)
    }
}

/// Safe, bounded parser for XML and INI style ASX and WVX playlists.
pub struct AsxPlaylistParser;

impl AsxPlaylistParser {
    /// Parses playlist content from raw bytes with a strict 1 MiB boundary check.
    pub fn parse_bytes(bytes: &[u8]) -> Result<AsxPlaylist, String> {
        if bytes.len() > MAX_ASX_PLAYLIST_BYTES {
            return Err(format!(
                "Playlist size ({} bytes) exceeds the allowed limit of {} bytes (1 MiB).",
                bytes.len(),
                MAX_ASX_PLAYLIST_BYTES
            ));
        }
        let content = std::str::from_utf8(bytes).map_err(|_| {
            "Playlist content is not valid UTF-8 or ASCII text.".to_string()
        })?;
        Self::parse_str(content)
    }

    /// Parses playlist content from a string slice with zero recursion or shell evaluation.
    pub fn parse_str(content: &str) -> Result<AsxPlaylist, String> {
        if content.len() > MAX_ASX_PLAYLIST_BYTES {
            return Err("Playlist content exceeds the 1 MiB limit.".to_string());
        }

        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err("Playlist content is empty.".to_string());
        }

        // Determine whether this is XML-style ASX or INI-style ASX
        if trimmed.starts_with('<') || trimmed.to_ascii_lowercase().contains("<asx") {
            Self::parse_xml_asx(trimmed)
        } else {
            Self::parse_ini_asx(trimmed)
        }
    }

    /// Bounded XML ASX/WVX parser without external entity resolution or recursion.
    fn parse_xml_asx(content: &str) -> Result<AsxPlaylist, String> {
        let mut playlist_title: Option<String> = None;
        let mut entries = Vec::new();

        // Extract playlist-level title (before first <entry>)
        let lower = content.to_ascii_lowercase();
        let first_entry_idx = lower.find("<entry");
        let header_slice = match first_entry_idx {
            Some(idx) => &content[..idx],
            None => content,
        };
        if let Some(title) = Self::extract_tag_content(header_slice, "title") {
            let t = title.trim();
            if !t.is_empty() {
                playlist_title = Some(t.to_string());
            }
        }

        // Scan for <entry> ... </entry> blocks
        let mut cursor = 0;
        while let Some(rel_start) = lower[cursor..].find("<entry") {
            if entries.len() >= MAX_ASX_ENTRIES {
                break;
            }
            let entry_start = cursor + rel_start;
            let entry_slice = match lower[entry_start..].find("</entry>") {
                Some(rel_end) => {
                    let end = entry_start + rel_end + "</entry>".len();
                    cursor = end;
                    &content[entry_start..end]
                }
                None => {
                    // Unclosed entry: take up to next <entry or end of content
                    let remaining = &content[entry_start..];
                    cursor = content.len();
                    remaining
                }
            };

            let entry_title = Self::extract_tag_content(entry_slice, "title")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            // Extract <ref href="...">
            if let Some(href) = Self::extract_ref_href(entry_slice) {
                let trimmed_href = href.trim().to_string();
                if !trimmed_href.is_empty() {
                    let scheme = trimmed_href
                        .find(':')
                        .map(|c| trimmed_href[..c].to_ascii_lowercase());
                    let is_fallback = matches!(scheme.as_deref(), Some("http") | Some("https"));
                    let is_mms = matches!(
                        scheme.as_deref(),
                        Some("mms") | Some("mmsh") | Some("mmst") | Some("mmsu")
                    );

                    entries.push(AsxEntry {
                        title: entry_title,
                        href: trimmed_href,
                        scheme,
                        is_supported_fallback: is_fallback,
                        is_mms,
                    });
                }
            }
        }

        // If no <entry> tags were found, look for standalone <ref href="..." /> tags
        if entries.is_empty() {
            let mut cursor = 0;
            while let Some(rel_idx) = lower[cursor..].find("<ref") {
                if entries.len() >= MAX_ASX_ENTRIES {
                    break;
                }
                let ref_start = cursor + rel_idx;
                let ref_end = match lower[ref_start..].find('>') {
                    Some(tag_end) => ref_start + tag_end + 1,
                    None => content.len(),
                };
                let tag_slice = &content[ref_start..ref_end];
                cursor = ref_end;

                if let Some(href) = Self::extract_ref_href(tag_slice) {
                    let trimmed_href = href.trim().to_string();
                    if !trimmed_href.is_empty() {
                        let scheme = trimmed_href
                            .find(':')
                            .map(|c| trimmed_href[..c].to_ascii_lowercase());
                        let is_fallback = matches!(scheme.as_deref(), Some("http") | Some("https"));
                        let is_mms = matches!(
                            scheme.as_deref(),
                            Some("mms") | Some("mmsh") | Some("mmst") | Some("mmsu")
                        );

                        entries.push(AsxEntry {
                            title: None,
                            href: trimmed_href,
                            scheme,
                            is_supported_fallback: is_fallback,
                            is_mms,
                        });
                    }
                }
            }
        }

        if entries.is_empty() {
            return Err("The ASX playlist does not contain any valid media references (<ref href=...>).".to_string());
        }

        Ok(AsxPlaylist {
            title: playlist_title,
            entries,
        })
    }

    /// Safe parser for INI-style ASX or WVX files (e.g. Ref1=http://...).
    fn parse_ini_asx(content: &str) -> Result<AsxPlaylist, String> {
        let mut entries = Vec::new();
        let mut playlist_title: Option<String> = None;

        for line in content.lines() {
            if entries.len() >= MAX_ASX_ENTRIES {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                continue;
            }

            if let Some(eq) = trimmed.find('=') {
                let key = trimmed[..eq].trim().to_ascii_lowercase();
                let val = trimmed[eq + 1..].trim();

                if key == "title" && playlist_title.is_none() {
                    playlist_title = Some(val.to_string());
                } else if key.starts_with("ref") || key.starts_with("file") {
                    let scheme = val.find(':').map(|c| val[..c].to_ascii_lowercase());
                    let is_fallback = matches!(scheme.as_deref(), Some("http") | Some("https"));
                    let is_mms = matches!(
                        scheme.as_deref(),
                        Some("mms") | Some("mmsh") | Some("mmst") | Some("mmsu")
                    );

                    entries.push(AsxEntry {
                        title: None,
                        href: val.to_string(),
                        scheme,
                        is_supported_fallback: is_fallback,
                        is_mms,
                    });
                }
            }
        }

        if entries.is_empty() {
            return Err("The playlist does not contain any valid media references.".to_string());
        }

        Ok(AsxPlaylist {
            title: playlist_title,
            entries,
        })
    }

    /// Safely extracts text inside `<tag>...</tag>`.
    fn extract_tag_content<'a>(slice: &'a str, tag_name: &str) -> Option<&'a str> {
        let lower = slice.to_ascii_lowercase();
        let open_tag = format!("<{tag_name}");
        let close_tag = format!("</{tag_name}>");

        let start_pos = lower.find(&open_tag)?;
        let tag_close_pos = slice[start_pos..].find('>')? + start_pos + 1;
        let end_pos = lower[tag_close_pos..].find(&close_tag)? + tag_close_pos;

        Some(&slice[tag_close_pos..end_pos])
    }

    /// Safely extracts the value of `href="..."` or `href='...'` inside a `<ref>` tag.
    fn extract_ref_href(tag_slice: &str) -> Option<String> {
        let lower = tag_slice.to_ascii_lowercase();
        let href_pos = lower.find("href")?;
        let after_href = &tag_slice[href_pos + 4..];
        let eq_pos = after_href.find('=')?;
        let after_eq = after_href[eq_pos + 1..].trim_start();

        if let Some(stripped) = after_eq.strip_prefix('"') {
            let quote_end = stripped.find('"')?;
            Some(stripped[..quote_end].to_string())
        } else if let Some(stripped) = after_eq.strip_prefix('\'') {
            let quote_end = stripped.find('\'')?;
            Some(stripped[..quote_end].to_string())
        } else {
            // Unquoted: take up to whitespace or > or /
            let end = after_eq
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(after_eq.len());
            Some(after_eq[..end].to_string())
        }
    }
}
