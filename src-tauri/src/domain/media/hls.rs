use std::collections::HashMap;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HlsByteRange {
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HlsSegment {
    pub url: String,
    pub byte_range: Option<HlsByteRange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HlsVariant {
    pub url: String,
    pub bandwidth: u64,
    pub resolution: Option<String>,
    pub audio_group: Option<String>,
    pub audio_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HlsMasterPlaylist {
    pub variants: Vec<HlsVariant>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HlsMediaPlaylist {
    pub urls: Vec<String>,
    pub segments: Vec<HlsSegment>,
    pub duration: f64,
    pub extension: &'static str,
    pub init_map: Option<String>,
    pub init_byte_range: Option<HlsByteRange>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedHls {
    Master(HlsMasterPlaylist),
    Media(HlsMediaPlaylist),
}

fn parse_attributes(line: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let text = if let Some((_, rest)) = line.split_once(':') {
        rest
    } else {
        line
    };
    let mut in_quotes = false;
    let mut current_key = String::new();
    let mut current_val = String::new();
    let mut reading_val = false;

    for ch in text.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == '=' && !in_quotes && !reading_val {
            reading_val = true;
        } else if ch == ',' && !in_quotes {
            let key = current_key.trim().to_uppercase();
            let val = current_val.trim().trim_matches('"').to_owned();
            if !key.is_empty() {
                attributes.insert(key, val);
            }
            current_key.clear();
            current_val.clear();
            reading_val = false;
        } else if reading_val {
            current_val.push(ch);
        } else {
            current_key.push(ch);
        }
    }
    let key = current_key.trim().to_uppercase();
    let val = current_val.trim().trim_matches('"').to_owned();
    if !key.is_empty() {
        attributes.insert(key, val);
    }
    attributes
}

fn resolve_url(relative_or_absolute: &str, base: &str) -> Result<String, String> {
    let base_url = Url::parse(base).map_err(|e| e.to_string())?;
    let resolved = base_url
        .join(relative_or_absolute)
        .map_err(|e| e.to_string())?;
    Ok(resolved.to_string())
}

fn parse_byte_range_str(
    val: &str,
    last_end: Option<u64>,
    same_resource: bool,
) -> Result<HlsByteRange, String> {
    let clean = val.trim().trim_matches('"');
    let (len_str, offset_str) = match clean.split_once('@') {
        Some((l, o)) => (l.trim(), Some(o.trim())),
        None => (clean, None),
    };

    let length = len_str
        .parse::<u64>()
        .map_err(|_| format!("Invalid HLS byte-range length: {len_str}"))?;

    let offset = if let Some(o_str) = offset_str {
        o_str
            .parse::<u64>()
            .map_err(|_| format!("Invalid HLS byte-range offset: {o_str}"))?
    } else if same_resource {
        last_end.unwrap_or(0)
    } else {
        0
    };

    Ok(HlsByteRange { offset, length })
}

pub fn parse_hls(text: &str, base: &str) -> Result<ParsedHls, String> {
    if !text.trim_start().starts_with("#EXTM3U") {
        return Err("The server did not return an HLS playlist.".to_owned());
    }
    let mut variants = Vec::new();
    let mut segments = Vec::new();
    let mut pending_variant: Option<HashMap<String, String>> = None;
    let mut pending_byte_range: Option<String> = None;
    let mut duration = 0.0_f64;
    let mut init_map: Option<String> = None;
    let mut init_byte_range: Option<HlsByteRange> = None;
    let mut encrypted = false;
    let mut has_endlist = false;
    let mut audio_groups: HashMap<String, String> = HashMap::new();

    let mut last_segment_url: Option<String> = None;
    let mut last_byte_range_end: Option<u64> = None;

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line == "#EXT-X-ENDLIST" {
            has_endlist = true;
        }
        if line.starts_with("#EXT-X-MEDIA:") {
            let attrs = parse_attributes(line);
            if attrs.get("TYPE").map(|v| v.as_str()) == Some("AUDIO") {
                if let (Some(uri), Some(group_id)) = (attrs.get("URI"), attrs.get("GROUP-ID")) {
                    let is_default = attrs.get("DEFAULT").map(|v| v.as_str()) == Some("YES");
                    if !audio_groups.contains_key(group_id) || is_default {
                        audio_groups.insert(group_id.clone(), resolve_url(uri, base)?);
                    }
                }
            }
        } else if line.starts_with("#EXT-X-STREAM-INF:") {
            pending_variant = Some(parse_attributes(line));
        } else if line.starts_with("#EXT-X-KEY:") {
            let attrs = parse_attributes(line);
            if attrs.get("METHOD").map(|v| v.as_str()) != Some("NONE") {
                encrypted = true;
            }
        } else if line.starts_with("#EXT-X-BYTERANGE:") {
            let range_str = line.strip_prefix("#EXT-X-BYTERANGE:").unwrap_or("").trim();
            pending_byte_range = Some(range_str.to_string());
        } else if line.starts_with("#EXT-X-MAP:") {
            let attrs = parse_attributes(line);
            if let Some(uri) = attrs.get("URI") {
                let resolved = resolve_url(uri, base)?;
                if let Some(existing) = &init_map {
                    if existing != &resolved {
                        return Err(
                            "HLS streams with changing initialization maps are not supported yet."
                                .to_owned(),
                        );
                    }
                }
                init_map = Some(resolved);
            }
            if let Some(br_str) = attrs.get("BYTERANGE") {
                init_byte_range = Some(parse_byte_range_str(br_str, None, false)?);
            }
        } else if line.starts_with("#EXTINF:") {
            let duration_str = line.strip_prefix("#EXTINF:").unwrap_or("");
            let duration_val = duration_str
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .parse::<f64>()
                .unwrap_or(0.0);
            duration += duration_val;
        } else if !line.starts_with('#') {
            let url = resolve_url(line, base)?;
            if let Some(variant_attrs) = pending_variant.take() {
                let bandwidth = variant_attrs
                    .get("BANDWIDTH")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let resolution = variant_attrs.get("RESOLUTION").cloned();
                let audio_group = variant_attrs.get("AUDIO").cloned();
                variants.push(HlsVariant {
                    url,
                    bandwidth,
                    resolution,
                    audio_group,
                    audio_url: None,
                });
            } else {
                let byte_range = if let Some(br_val) = pending_byte_range.take() {
                    let same_res = last_segment_url.as_deref() == Some(&url);
                    let parsed_range = parse_byte_range_str(&br_val, last_byte_range_end, same_res)?;
                    last_byte_range_end = Some(parsed_range.offset + parsed_range.length);
                    last_segment_url = Some(url.clone());
                    Some(parsed_range)
                } else {
                    last_segment_url = Some(url.clone());
                    last_byte_range_end = None;
                    None
                };

                segments.push(HlsSegment {
                    url,
                    byte_range,
                });
            }
        }
    }

    if encrypted {
        return Err("Encrypted HLS is not supported by this build.".to_owned());
    }

    if !variants.is_empty() {
        for variant in &mut variants {
            if let Some(audio_group) = &variant.audio_group {
                variant.audio_url = audio_groups.get(audio_group).cloned();
            }
        }
        variants.sort_unstable_by(|a, b| b.bandwidth.cmp(&a.bandwidth));
        return Ok(ParsedHls::Master(HlsMasterPlaylist { variants }));
    }

    if !has_endlist {
        return Err("Live HLS streams are not supported yet. Choose an on-demand video.".to_owned());
    }

    if segments.is_empty() || segments.len() > 20_000 {
        return Err(
            "HLS playlist has no segments or exceeds the 20,000 segment limit.".to_owned(),
        );
    }

    let extension = if init_map.is_some() { ".mp4" } else { ".ts" };
    let mut urls = Vec::new();
    let mut all_segments = Vec::new();

    if let Some(init) = &init_map {
        urls.push(init.clone());
        all_segments.push(HlsSegment {
            url: init.clone(),
            byte_range: init_byte_range,
        });
    }

    for seg in &segments {
        urls.push(seg.url.clone());
    }
    all_segments.extend(segments);

    Ok(ParsedHls::Media(HlsMediaPlaylist {
        urls,
        segments: all_segments,
        duration,
        extension,
        init_map,
        init_byte_range,
    }))
}

pub fn scoped_headers(
    headers: &HashMap<String, String>,
    source: &str,
    target: &str,
) -> HashMap<String, String> {
    let mut next = headers.clone();
    if let (Ok(source_url), Ok(target_url)) = (Url::parse(source), Url::parse(target)) {
        if source_url.origin() != target_url.origin() {
            next.remove("cookie");
            next.remove("Cookie");
            next.remove("authorization");
            next.remove("Authorization");
            next.remove("origin");
            next.remove("Origin");
        }
    }
    next
}
