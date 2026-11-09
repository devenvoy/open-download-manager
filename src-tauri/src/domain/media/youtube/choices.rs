// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize youtube domain module into focused submodules
// Description: YouTube format extraction and pairing logic for audio/video streams.

use std::collections::HashMap;

use super::{types::YouTubeFormatChoice, url::valid_format_id};

pub fn choices_from_info(
    info: &serde_json::Value,
    url: &str,
) -> Result<Vec<YouTubeFormatChoice>, String> {
    if info.get("is_live").and_then(|v| v.as_bool()) == Some(true)
        || info.get("live_status").and_then(|v| v.as_str()) == Some("is_live")
    {
        return Err("Live streams are not supported yet.".to_owned());
    }

    let formats = info
        .get("formats")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();

    let valid_formats: Vec<_> = formats
        .into_iter()
        .filter(|f| {
            let has_drm = f.get("has_drm").and_then(|v| v.as_bool()).unwrap_or(false);
            let has_url = f.get("url").and_then(|v| v.as_str()).is_some();
            let protocol = f.get("protocol").and_then(|v| v.as_str()).unwrap_or("");
            !has_drm
                && has_url
                && matches!(
                    protocol,
                    "https" | "http" | "m3u8_native" | "m3u8" | "http_dash_segments"
                )
        })
        .collect();

    let mut audios: Vec<_> = valid_formats
        .iter()
        .filter(|f| {
            let vcodec = f.get("vcodec").and_then(|v| v.as_str()).unwrap_or("");
            let acodec = f.get("acodec").and_then(|v| v.as_str()).unwrap_or("");
            let ext = f.get("ext").and_then(|v| v.as_str()).unwrap_or("");
            let format_id = f.get("format_id").and_then(|v| v.as_str()).unwrap_or("");
            vcodec == "none" && acodec != "none" && ext == "m4a" && !format_id.contains("drc")
        })
        .collect();

    audios.sort_by(|a, b| {
        let abr_b = b.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0) as u64;
        let abr_a = a.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0) as u64;
        abr_b.cmp(&abr_a)
    });
    let best_audio = audios.first().copied();

    let mut groups: HashMap<u64, &serde_json::Value> = HashMap::new();
    for f in &valid_formats {
        let vcodec = f.get("vcodec").and_then(|v| v.as_str()).unwrap_or("");
        let acodec = f.get("acodec").and_then(|v| v.as_str()).unwrap_or("");
        let ext = f.get("ext").and_then(|v| v.as_str()).unwrap_or("");
        let height = f.get("height").and_then(|v| v.as_u64());

        if vcodec.is_empty() || vcodec == "none" || ext != "mp4" || height.is_none() {
            continue;
        }
        let height = height.unwrap();
        if acodec == "none" && best_audio.is_none() {
            continue;
        }

        let score = |v: &serde_json::Value| -> u64 {
            let proto = v.get("protocol").and_then(|p| p.as_str()).unwrap_or("");
            let vc = v.get("vcodec").and_then(|c| c.as_str()).unwrap_or("");
            let fps = v.get("fps").and_then(|f| f.as_f64()).unwrap_or(0.0) as u64;
            (if proto == "https" { 100 } else { 0 })
                + (if vc.starts_with("avc") { 50 } else { 0 })
                + fps / 100
        };

        match groups.get(&height) {
            None => {
                groups.insert(height, f);
            }
            Some(old) if score(f) > score(old) => {
                groups.insert(height, f);
            }
            _ => {}
        }
    }

    let title_raw = info.get("title").and_then(|t| t.as_str()).unwrap_or("Online video");
    let title: String = title_raw.chars().take(180).collect();
    let duration = info.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);

    let mut sorted_videos: Vec<_> = groups.into_iter().collect();
    sorted_videos.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let audio_id = best_audio
        .and_then(|a| a.get("format_id"))
        .and_then(|id| id.as_str())
        .unwrap_or("");
    let audio_size = best_audio
        .and_then(|a| {
            a.get("filesize")
                .or_else(|| a.get("filesize_approx"))
                .and_then(|s| s.as_u64())
        })
        .unwrap_or(0);

    let mut result = Vec::with_capacity(sorted_videos.len() + 1);
    for (height, f) in sorted_videos {
        let paired = f.get("acodec").and_then(|a| a.as_str()) == Some("none");
        let format_id = f.get("format_id").and_then(|id| id.as_str()).unwrap_or("");
        if !valid_format_id(format_id) || (paired && !valid_format_id(audio_id)) {
            continue;
        }
        let youtube_format = if paired && !audio_id.is_empty() {
            format!("{format_id}+{audio_id}")
        } else {
            format_id.to_owned()
        };
        let fps = f.get("fps").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
        let video_size = f
            .get("filesize")
            .or_else(|| f.get("filesize_approx"))
            .and_then(|s| s.as_u64())
            .unwrap_or(0);
        let estimated_size = video_size + (if paired { audio_size } else { 0 });

        let audio_stream_url = if paired {
            best_audio.and_then(|a| a.get("url")).and_then(|u| u.as_str())
        } else {
            None
        };

        result.push(YouTubeFormatChoice {
            url: url.to_owned(),
            provider: "youtube".to_owned(),
            youtube_format,
            format: "MP4".to_owned(),
            quality: format!("{height}p"),
            fps,
            audio_state: "Video + audio".to_owned(),
            title: title.clone(),
            output_format: "mp4".to_owned(),
            estimated_size,
            duration,
            audio_url: audio_stream_url.map(|s| s.to_owned()),
        });
    }

    if let Some(audio) = best_audio {
        let audio_format_id = audio.get("format_id").and_then(|id| id.as_str()).unwrap_or("");
        if valid_format_id(audio_format_id) {
            result.push(YouTubeFormatChoice {
                url: url.to_owned(),
                provider: "youtube".to_owned(),
                youtube_format: format!("audio:{audio_format_id}"),
                format: "MP3".to_owned(),
                quality: String::new(),
                fps: 0,
                audio_state: "Audio only".to_owned(),
                title,
                output_format: "mp3".to_owned(),
                estimated_size: audio_size,
                duration,
                audio_url: None,
            });
        }
    }

    if result.is_empty() {
        return Err(
            "No downloadable video formats were found. The page may require sign-in, use DRM, or expose an unsupported format."
                .to_owned(),
        );
    }

    Ok(result)
}
