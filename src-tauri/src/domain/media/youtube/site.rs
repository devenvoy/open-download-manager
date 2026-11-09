// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize youtube domain module into focused submodules
// Description: Generic video site format extraction, track pairing, and ranking logic.

use std::collections::HashMap;

use super::{types::YouTubeFormatChoice, url::valid_format_id};

pub fn site_choices_from_info(
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
        .and_then(|formats| formats.as_array())
        .cloned()
        .unwrap_or_default();
    let valid_formats: Vec<_> = formats
        .iter()
        .filter(|format| {
            let has_drm = format
                .get("has_drm")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let has_url = format.get("url").and_then(|value| value.as_str()).is_some();
            let protocol = format
                .get("protocol")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            !has_drm
                && has_url
                && matches!(
                    protocol,
                    "https" | "http" | "m3u8_native" | "m3u8" | "http_dash_segments"
                )
        })
        .collect();

    let find_audio = |video_ext: &str| {
        valid_formats
            .iter()
            .copied()
            .filter(|format| {
                let vcodec = format
                    .get("vcodec")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let acodec = format
                    .get("acodec")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let ext = format
                    .get("ext")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                vcodec == "none"
                    && !acodec.is_empty()
                    && acodec != "none"
                    && match video_ext {
                        "mp4" => matches!(ext, "m4a" | "mp4"),
                        "webm" => ext == "webm",
                        _ => false,
                    }
            })
            .max_by(|a, b| {
                let abr = |format: &&serde_json::Value| {
                    format
                        .get("abr")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0)
                };
                abr(a).total_cmp(&abr(b))
            })
    };

    let mut groups: HashMap<(u64, String), &serde_json::Value> = HashMap::new();
    for format in &valid_formats {
        let vcodec = format
            .get("vcodec")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let acodec = format
            .get("acodec")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let ext = format
            .get("ext")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let Some(height) = format.get("height").and_then(|value| value.as_u64()) else {
            continue;
        };
        if vcodec.is_empty()
            || vcodec == "none"
            || !matches!(ext, "mp4" | "webm")
            || acodec.is_empty()
            || (acodec == "none" && find_audio(ext).is_none())
        {
            continue;
        }

        let key = (height, ext.to_owned());
        let score = |candidate: &serde_json::Value| {
            let bitrate = candidate
                .get("tbr")
                .or_else(|| candidate.get("vbr"))
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            let fps = candidate
                .get("fps")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            (bitrate, fps)
        };
        match groups.get(&key) {
            None => {
                groups.insert(key, format);
            }
            Some(previous) if score(format) > score(previous) => {
                groups.insert(key, format);
            }
            _ => {}
        }
    }

    let title: String = info
        .get("title")
        .and_then(|title| title.as_str())
        .unwrap_or("Online video")
        .chars()
        .take(180)
        .collect();
    let duration = info
        .get("duration")
        .and_then(|duration| duration.as_f64())
        .unwrap_or(0.0);
    let mut videos: Vec<_> = groups.into_iter().collect();
    videos.sort_by(|((height_a, ext_a), _), ((height_b, ext_b), _)| {
        height_b.cmp(height_a).then_with(|| ext_a.cmp(ext_b))
    });

    let mut choices = Vec::with_capacity(videos.len() + 1);
    for ((height, ext), video) in videos {
        let video_id = video
            .get("format_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if !valid_format_id(video_id) {
            continue;
        }
        let paired = video
            .get("acodec")
            .and_then(|value| value.as_str())
            == Some("none");
        let audio = paired.then(|| find_audio(&ext)).flatten();
        let audio_id = audio
            .and_then(|format| format.get("format_id"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if paired && !valid_format_id(audio_id) {
            continue;
        }
        let selector = if paired {
            format!("{video_id}+{audio_id}")
        } else {
            video_id.to_owned()
        };
        let size = |format: &serde_json::Value| {
            format
                .get("filesize")
                .or_else(|| format.get("filesize_approx"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
        };
        let fps = video
            .get("fps")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0) as u32;
        let audio_stream_url = audio.and_then(|a| a.get("url")).and_then(|u| u.as_str());
        choices.push(YouTubeFormatChoice {
            url: url.to_owned(),
            provider: "extractor".to_owned(),
            youtube_format: selector,
            format: ext.to_ascii_uppercase(),
            quality: format!("{height}p"),
            fps,
            audio_state: "Video + audio".to_owned(),
            title: title.clone(),
            output_format: ext,
            estimated_size: size(video) + audio.map(size).unwrap_or(0),
            duration,
            audio_url: audio_stream_url.map(|s| s.to_owned()),
        });
    }

    let best_audio = valid_formats
        .iter()
        .copied()
        .filter(|format| {
            let vcodec = format
                .get("vcodec")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let acodec = format
                .get("acodec")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            vcodec == "none" && !acodec.is_empty() && acodec != "none"
        })
        .max_by(|a, b| {
            let abr = |format: &&serde_json::Value| {
                format
                    .get("abr")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0)
            };
            abr(a).total_cmp(&abr(b))
        });
    if let Some(audio) = best_audio {
        let audio_id = audio
            .get("format_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if valid_format_id(audio_id) {
            let estimated_size = audio
                .get("filesize")
                .or_else(|| audio.get("filesize_approx"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            choices.push(YouTubeFormatChoice {
                url: url.to_owned(),
                provider: "extractor".to_owned(),
                youtube_format: format!("audio:{audio_id}"),
                format: "MP3".to_owned(),
                quality: String::new(),
                fps: 0,
                audio_state: "Audio only".to_owned(),
                title,
                output_format: "mp3".to_owned(),
                estimated_size,
                duration,
                audio_url: None,
            });
        }
    }

    if choices.is_empty() {
        return Err(
            "No downloadable video formats were found. The page may require sign-in, use DRM, or expose an unsupported format."
                .to_owned(),
        );
    }
    Ok(choices)
}
