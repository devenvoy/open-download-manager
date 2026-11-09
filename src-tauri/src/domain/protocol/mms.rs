// Created: 2026-11-08
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: Domain protocol detector, session state machine, and safe bounded ASX/WVX playlist parser for MMS.

use serde::{Deserialize, Serialize};

/// Maximum allowed playlist file size in bytes to prevent memory blowup (1 MiB).
pub const MAX_ASX_PLAYLIST_BYTES: usize = 1_048_576;

/// Maximum number of media entries parsed from an ASX/WVX playlist.
pub const MAX_ASX_ENTRIES: usize = 500;

/// Diagnostic marker strings reflecting the MMS transport stages.
pub const MMS_STAGE_CONNECTING: &str = "MMSS:TrStNCn";
pub const MMS_STAGE_HANDSHAKING: &str = "MMSSss:SndAlCnInf";

/// Variants of the Microsoft Media Server (MMS) protocol family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MmsVariant {
    /// Modern MMS over HTTP framing (supported via dedicated chunk session executor)
    Mmsh,
    /// Generic MMS protocol identifier (negotiated via MMSH session transport)
    Mms,
    /// Obsolete MMS over TCP port 1755 (deprecated by Microsoft, unsupported)
    Mmst,
    /// Obsolete MMS over UDP port 1755 (deprecated by Microsoft, unsupported)
    Mmsu,
}

impl MmsVariant {
    /// Returns the scheme identifier as a static string.
    pub fn scheme_str(&self) -> &'static str {
        match self {
            Self::Mmsh => "mmsh",
            Self::Mms => "mms",
            Self::Mmst => "mmst",
            Self::Mmsu => "mmsu",
        }
    }

    /// Whether this MMS variant is viable and supported via the dedicated MMS session executor.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Mmsh | Self::Mms)
    }

    /// Actionable migration explanation for unsupported legacy variants.
    pub fn unsupported_guidance(&self) -> &'static str {
        match self {
            Self::Mmst => {
                "MMST (MMS over TCP port 1755) is an obsolete, unmaintained proprietary protocol \
                 deprecated by Microsoft. Direct MMST connections are unsupported due to security \
                 and protocol obsolescence. Please use an HTTP, HTTPS, or RTSP media source, or \
                 extract the stream URL from an ASX/WVX playlist."
            }
            Self::Mmsu => {
                "MMSU (MMS over UDP port 1755) is an obsolete, unmaintained proprietary protocol \
                 deprecated by Microsoft. Direct MMSU connections are unsupported due to security \
                 risks and lack of flow control. Please use an HTTP, HTTPS, or RTSP media source, \
                 or extract the stream URL from an ASX/WVX playlist."
            }
            Self::Mms | Self::Mmsh => "MMS is supported via dedicated MMS streaming session.",
        }
    }
}

/// Protocol boundary detector for identifying MMS schemes and playlist wrappers during URL intake.
pub struct MmsProtocolDetector;

impl MmsProtocolDetector {
    /// Detects whether the scheme corresponds to an MMS protocol variant.
    pub fn detect_scheme(url_str: &str) -> Option<MmsVariant> {
        let trimmed = url_str.trim();
        let colon = trimmed.find(':')?;
        let scheme = &trimmed[..colon].to_ascii_lowercase();
        match scheme.as_str() {
            "mms" => Some(MmsVariant::Mms),
            "mmsh" => Some(MmsVariant::Mmsh),
            "mmst" => Some(MmsVariant::Mmst),
            "mmsu" => Some(MmsVariant::Mmsu),
            _ => None,
        }
    }

    /// Returns true if the URL has an MMS scheme (mms://, mmsh://, mmst://, mmsu://).
    pub fn is_mms(url_str: &str) -> bool {
        Self::detect_scheme(url_str).is_some()
    }

    /// Detects whether the URL points to an ASX or WVX Windows Media playlist wrapper.
    pub fn is_playlist_wrapper(url_str: &str) -> bool {
        let trimmed = url_str.trim();
        let path = trimmed.split('?').next().unwrap_or(trimmed);
        let path_lower = path.to_ascii_lowercase();
        path_lower.ends_with(".asx")
            || path_lower.ends_with(".wvx")
            || path_lower.ends_with(".wmx")
            || path_lower.ends_with(".wax")
    }

    /// Validates an incoming URL for MMS compatibility or yields clear actionable migration guidance.
    pub fn validate_or_explain(url_str: &str) -> Result<MmsVariant, String> {
        if Self::is_playlist_wrapper(url_str) {
            return Err(
                "ASX and WVX playlists are stream redirector wrappers, not direct media files. \
                 Extract the embedded stream URL using the playlist parser or provide the direct media link."
                    .to_string(),
            );
        }

        match Self::detect_scheme(url_str) {
            Some(variant) => {
                if variant.is_supported() {
                    Ok(variant)
                } else {
                    Err(variant.unsupported_guidance().to_string())
                }
            }
            None => Err("The provided address does not use an MMS streaming protocol.".to_string()),
        }
    }
}

/// Explicit connection and session states for the MMS streaming lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "detail")]
pub enum MmsSessionState {
    /// Session created, not yet connected.
    Initial,
    /// Transport connection starting (MMSS:TrStNCn).
    Connecting { endpoint: String },
    /// Session handshake and connection info negotiation (MMSSss:SndAlCnInf).
    Handshaking { stream_offset: u64 },
    /// Active media chunk streaming.
    Streaming {
        bytes_received: u64,
        chunks_processed: u64,
    },
    /// Stream completed cleanly (ASF $E chunk or EOF reached).
    Completed { total_bytes: u64 },
    /// Transfer explicitly cancelled by user.
    Cancelled,
    /// Midstream disconnection; eligible for bounded reconnect.
    Disconnected {
        bytes_at_disconnect: u64,
        reason: String,
    },
    /// Fatal session failure.
    Failed { error: String },
}

/// Dedicated MMS session state machine enforcing state boundaries and diagnostic tracking.
#[derive(Clone, Debug)]
pub struct MmsSessionStateMachine {
    state: MmsSessionState,
    diagnostic_events: Vec<String>,
}

impl Default for MmsSessionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl MmsSessionStateMachine {
    /// Creates a new MMS state machine in the Initial state.
    pub fn new() -> Self {
        Self {
            state: MmsSessionState::Initial,
            diagnostic_events: vec!["MMS:Initial".to_string()],
        }
    }

    /// Transitions to the Connecting state (`MMSS:TrStNCn`).
    pub fn start_connection(&mut self, endpoint: &str) {
        self.diagnostic_events
            .push(format!("{MMS_STAGE_CONNECTING}:{endpoint}"));
        self.state = MmsSessionState::Connecting {
            endpoint: endpoint.to_string(),
        };
    }

    /// Transitions to the Handshaking state (`MMSSss:SndAlCnInf`).
    pub fn send_connection_info(&mut self, stream_offset: u64) {
        self.diagnostic_events
            .push(format!("{MMS_STAGE_HANDSHAKING}:offset={stream_offset}"));
        self.state = MmsSessionState::Handshaking { stream_offset };
    }

    /// Transitions to the active Streaming state.
    pub fn begin_streaming(&mut self) {
        self.diagnostic_events.push("MMS:StreamingStarted".to_string());
        self.state = MmsSessionState::Streaming {
            bytes_received: 0,
            chunks_processed: 0,
        };
    }

    /// Updates byte counts during chunk processing.
    pub fn record_chunk(&mut self, chunk_bytes: usize) {
        if let MmsSessionState::Streaming {
            ref mut bytes_received,
            ref mut chunks_processed,
        } = self.state
        {
            *bytes_received += chunk_bytes as u64;
            *chunks_processed += 1;
        }
    }

    /// Transitions to the Completed state.
    pub fn complete(&mut self, total_bytes: u64) {
        self.diagnostic_events
            .push(format!("MMS:Completed:total={total_bytes}"));
        self.state = MmsSessionState::Completed { total_bytes };
    }

    /// Transitions to the Disconnected state for reconnect evaluation.
    pub fn disconnect(&mut self, reason: &str) {
        let bytes = match &self.state {
            MmsSessionState::Streaming { bytes_received, .. } => *bytes_received,
            _ => 0,
        };
        self.diagnostic_events
            .push(format!("MMS:Disconnected:{reason}:bytes={bytes}"));
        self.state = MmsSessionState::Disconnected {
            bytes_at_disconnect: bytes,
            reason: reason.to_string(),
        };
    }

    /// Transitions to the Cancelled state.
    pub fn cancel(&mut self) {
        self.diagnostic_events.push("MMS:Cancelled".to_string());
        self.state = MmsSessionState::Cancelled;
    }

    /// Transitions to the Failed state.
    pub fn fail(&mut self, error: &str) {
        self.diagnostic_events
            .push(format!("MMS:Failed:{error}"));
        self.state = MmsSessionState::Failed {
            error: error.to_string(),
        };
    }

    /// Returns the current session state.
    pub fn state(&self) -> &MmsSessionState {
        &self.state
    }

    /// Returns recorded diagnostic trace events.
    pub fn diagnostic_events(&self) -> &[String] {
        &self.diagnostic_events
    }
}

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
