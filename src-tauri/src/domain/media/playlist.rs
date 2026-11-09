// Created: 2026-11-05
// Reason: feat(playlist): implement YouTube playlist discovery, model, and batch queue controls (#21)
// Last updated: 2026-11-06
// Reason: perf: eliminate upfront clone in remove_completed, fuse HLS parsing and stream playlist entries
// Description: Batch playlist models representing multi-video collections, individual tracks, and metadata.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::category::DownloadCategory;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubePlaylistTarget {
    pub playlist_id: String,
    pub canonical_playlist_url: String,
    pub video_id: Option<String>,
    pub index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
    pub url: String,
    pub thumbnail: Option<String>,
    pub channel: Option<String>,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMetadata {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub uploader: Option<String>,
    pub webpage_url: String,
    pub entry_count: usize,
    pub entries: Vec<PlaylistEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistBatchItemRequest {
    pub id: String,
    pub url: String,
    pub title: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPlaylistDownloadsRequest {
    pub playlist_id: String,
    pub playlist_title: String,
    pub items: Vec<PlaylistBatchItemRequest>,
    pub common_category: Option<DownloadCategory>,
    pub common_directory: Option<String>,
}

pub fn parse_youtube_playlist_url(value: &str) -> Option<YouTubePlaylistTarget> {
    let url = Url::parse(value.trim()).ok()?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return None;
    }
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }

    let host = url.host_str()?.to_ascii_lowercase();
    let is_youtube_host = matches!(
        host.as_str(),
        "youtube.com"
            | "www.youtube.com"
            | "m.youtube.com"
            | "music.youtube.com"
            | "youtu.be"
    );

    if !is_youtube_host {
        return None;
    }

    let mut playlist_id: Option<String> = None;
    let mut video_id: Option<String> = None;
    let mut index: Option<u32> = None;

    if host == "youtu.be" {
        let path = url.path().trim_start_matches('/');
        if path.len() == 11 && is_valid_youtube_token(path) {
            video_id = Some(path.to_owned());
        }
    } else if url.path() == "/watch" {
        for (k, v) in url.query_pairs() {
            if k == "v" && v.len() == 11 && is_valid_youtube_token(&v) {
                video_id = Some(v.into_owned());
            }
        }
    }

    for (k, v) in url.query_pairs() {
        if k == "list" {
            let id = v.trim();
            if is_valid_playlist_id(id) {
                playlist_id = Some(id.to_owned());
            }
        } else if k == "index" {
            if let Ok(idx) = v.parse::<u32>() {
                index = Some(idx);
            }
        }
    }

    let playlist_id = playlist_id?;
    let canonical_playlist_url = format!("https://www.youtube.com/playlist?list={playlist_id}");

    Some(YouTubePlaylistTarget {
        playlist_id,
        canonical_playlist_url,
        video_id,
        index,
    })
}

fn is_valid_playlist_id(id: &str) -> bool {
    let len = id.len();
    if len < 10 || len > 64 {
        return false;
    }
    is_valid_youtube_token(id)
}

fn is_valid_youtube_token(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn parse_playlist_json(
    info: &serde_json::Value,
    fallback_url: &str,
) -> Result<PlaylistMetadata, String> {
    let id = info
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
        .ok_or_else(|| "The playlist response did not contain a valid playlist ID.".to_owned())?;

    let title_raw = info
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("YouTube Playlist");
    let title: String = title_raw.trim().chars().take(300).collect();

    let description = info
        .get("description")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().chars().take(1000).collect());

    let uploader = info
        .get("uploader")
        .or_else(|| info.get("channel"))
        .or_else(|| info.get("uploader_id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().chars().take(200).collect());

    let webpage_url = info
        .get("webpage_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| {
            if fallback_url.is_empty() {
                format!("https://www.youtube.com/playlist?list={id}")
            } else {
                fallback_url.to_owned()
            }
        });

    let empty_entries = Vec::new();
    let raw_entries = info
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&empty_entries);

    const MAX_PLAYLIST_ENTRIES: usize = 500;
    let count = raw_entries.len().min(MAX_PLAYLIST_ENTRIES);
    let mut entries = Vec::with_capacity(count);

    for (idx, entry_val) in raw_entries.iter().take(MAX_PLAYLIST_ENTRIES).enumerate() {
        let entry_id = entry_val
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| s.len() == 11 && is_valid_youtube_token(s))
            .map(|s| s.to_owned());

        let entry_id = match entry_id {
            Some(id) => id,
            None => continue,
        };

        let raw_title = entry_val
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Skip private or deleted videos
        if raw_title == "[Private video]"
            || raw_title == "[Deleted video]"
            || raw_title.trim().is_empty()
        {
            continue;
        }
        let entry_title: String = raw_title.trim().chars().take(300).collect();

        let duration = entry_val
            .get("duration")
            .and_then(|v| v.as_f64())
            .filter(|&d| d > 0.0);

        let url = entry_val
            .get("url")
            .and_then(|v| v.as_str())
            .and_then(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    Some(u.to_owned())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={entry_id}"));

        let thumbnail = entry_val
            .get("thumbnails")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.last())
            .and_then(|thumb| thumb.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .or_else(|| {
                entry_val
                    .get("thumbnail")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
            })
            .or_else(|| {
                Some(format!("https://i.ytimg.com/vi/{entry_id}/hqdefault.jpg"))
            });

        let channel = entry_val
            .get("channel")
            .or_else(|| entry_val.get("uploader"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().chars().take(200).collect());

        entries.push(PlaylistEntry {
            id: entry_id,
            title: entry_title,
            duration,
            url,
            thumbnail,
            channel,
            index: idx + 1,
        });
    }

    let entry_count = entries.len();
    if entry_count == 0 {
        return Err("No downloadable videos found in this playlist (videos may be private or unavailable).".to_owned());
    }

    Ok(PlaylistMetadata {
        id,
        title,
        description,
        uploader,
        webpage_url,
        entry_count,
        entries,
    })
}
