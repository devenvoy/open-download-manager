use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeFormatChoice {
    pub url: String,
    pub provider: String,
    pub youtube_format: String,
    pub format: String,
    pub quality: String,
    pub fps: u32,
    pub audio_state: String,
    pub title: String,
    pub output_format: String,
    pub estimated_size: u64,
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
}

pub fn youtube_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let video_id = if host == "youtu.be" {
        url.path().strip_prefix('/')?.to_owned()
    } else if matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com"
    ) {
        if url.path() == "/watch" {
            url.query_pairs().find(|(k, _)| k == "v")?.1.to_string()
        } else if let Some(stripped) = url
            .path()
            .strip_prefix("/shorts/")
            .or_else(|| url.path().strip_prefix("/embed/"))
        {
            stripped.split('/').next()?.to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };

    let is_valid_id = video_id.len() == 11
        && video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_valid_id {
        Some(format!("https://www.youtube.com/watch?v={video_id}"))
    } else {
        None
    }
}

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

fn valid_format_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

pub fn youtube_error(raw: &str) -> (String, u16, &'static str) {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("http error 429") || lower.contains("too many requests") {
        return (
            "YouTube is temporarily refusing requests (too many requests). Wait before trying again. Repeated clicks will not help; this app does not currently support YouTube sign-in verification."
                .to_owned(),
            429,
            "YOUTUBE_RATE_LIMIT",
        );
    }
    if lower.contains("confirm you are not a bot")
        || lower.contains("sign in")
        || lower.contains("login required")
        || lower.contains("cookies-from-browser")
    {
        return (
            "YouTube requires sign-in or browser verification for this request. The downloader cannot currently complete that verification."
                .to_owned(),
            422,
            "YOUTUBE_VERIFICATION",
        );
    }

    let redacted_lines: Vec<String> = raw
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|token| {
                    if token.starts_with("http://") || token.starts_with("https://") {
                        "[URL]"
                    } else {
                        token
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let error_line = redacted_lines
        .iter()
        .rev()
        .find(|line| line.starts_with("ERROR:"))
        .map(|s| s.as_str())
        .unwrap_or(raw);

    let message: String = error_line.trim().chars().take(700).collect();
    let final_message = if message.is_empty() {
        "Video download failed.".to_owned()
    } else {
        message
    };
    (final_message, 400, "YOUTUBE_ERROR")
}
