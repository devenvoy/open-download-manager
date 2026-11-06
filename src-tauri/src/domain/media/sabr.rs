use base64::prelude::*;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrAudioTrack {
    pub id: Option<String>,
    #[serde(default)]
    pub audio_is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrColorInfo {
    pub transfer_characteristics: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrFormat {
    pub itag: u32,
    pub mime_type: String,
    #[serde(default)]
    pub bitrate: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_bitrate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channels: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_length: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approx_duration_ms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_duration_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xtags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_track: Option<SabrAudioTrack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_info: Option<SabrColorInfo>,
    #[serde(default)]
    pub is_drc: bool,
    #[serde(default)]
    pub is_vb: bool,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default)]
    pub has_video: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_track_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrClientInfo {
    #[serde(default = "default_client_name")]
    pub client_name: u32,
    #[serde(default)]
    pub client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_make: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
}

fn default_client_name() -> u32 {
    1
}

impl Default for SabrClientInfo {
    fn default() -> Self {
        Self {
            client_name: 1,
            client_version: String::new(),
            hl: None,
            device_make: None,
            device_model: None,
            os_name: None,
            os_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrSession {
    pub video_id: String,
    pub server_abr_streaming_url: String,
    pub video_playback_ustreamer_config: String,
    #[serde(default)]
    pub client_info: Option<SabrClientInfo>,
    pub formats: Vec<SabrFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_itag: Option<u32>,
    pub audio_itag: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub po_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSabrSession {
    pub video_id: String,
    pub server_abr_streaming_url: String,
    pub video_playback_ustreamer_config: String,
    pub client_info: SabrClientInfo,
    pub formats: Vec<SabrFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_itag: Option<u32>,
    pub audio_itag: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub po_token: Option<String>,
}

impl NormalizedSabrSession {
    pub fn to_session(&self) -> SabrSession {
        SabrSession {
            video_id: self.video_id.clone(),
            server_abr_streaming_url: self.server_abr_streaming_url.clone(),
            video_playback_ustreamer_config: self.video_playback_ustreamer_config.clone(),
            client_info: Some(self.client_info.clone()),
            formats: self.formats.clone(),
            video_itag: self.video_itag,
            audio_itag: self.audio_itag,
            request_body: self.request_body.clone(),
            po_token: self.po_token.clone(),
        }
    }
}

pub fn google_video_sabr_url(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "Invalid YouTube SABR URL.".to_owned())?;
    if url.scheme() != "https" {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if host != "googlevideo.com" && !host.ends_with(".googlevideo.com") {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    if url.path() != "/videoplayback" {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    if url.query_pairs().find(|(k, v)| k == "sabr" && v == "1").is_none() {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    Ok(url.to_string())
}

#[derive(Default)]
pub struct ParsedSabrSeed {
    pub client_info: Option<SabrClientInfo>,
    pub po_token: Option<Vec<u8>>,
}

pub struct ProtoReader<'a> {
    pub data: &'a [u8],
}

impl<'a> ProtoReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn read_varint(&mut self) -> Result<u64, String> {
        let mut result = 0u64;
        let mut shift = 0;
        while !self.data.is_empty() {
            let byte = self.data[0];
            self.data = &self.data[1..];
            result |= ((byte & 0x7F) as u64) << shift;
            if (byte & 0x80) == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                return Err("Varint overflow in protobuf.".to_owned());
            }
        }
        Err("Unexpected end of protobuf varint.".to_owned())
    }

    pub fn read_tag(&mut self) -> Result<(u32, u8), String> {
        let varint = self.read_varint()?;
        let field_number = (varint >> 3) as u32;
        let wire_type = (varint & 0x07) as u8;
        Ok((field_number, wire_type))
    }

    pub fn read_length_delimited(&mut self) -> Result<&'a [u8], String> {
        let len = self.read_varint()? as usize;
        if self.data.len() < len {
            return Err("Unexpected end of length-delimited protobuf field.".to_owned());
        }
        let (bytes, rest) = self.data.split_at(len);
        self.data = rest;
        Ok(bytes)
    }

    pub fn skip_field(&mut self, wire_type: u8) -> Result<(), String> {
        match wire_type {
            0 => {
                let _ = self.read_varint()?;
                Ok(())
            }
            1 => {
                if self.data.len() < 8 {
                    return Err("Unexpected end of 64-bit protobuf field.".to_owned());
                }
                self.data = &self.data[8..];
                Ok(())
            }
            2 => {
                let _ = self.read_length_delimited()?;
                Ok(())
            }
            5 => {
                if self.data.len() < 4 {
                    return Err("Unexpected end of 32-bit protobuf field.".to_owned());
                }
                self.data = &self.data[4..];
                Ok(())
            }
            _ => Err(format!("Unsupported protobuf wire type: {wire_type}")),
        }
    }
}

pub fn encode_varint(mut val: u64, buf: &mut Vec<u8>) {
    while val >= 0x80 {
        buf.push(((val & 0x7F) as u8) | 0x80);
        val >>= 7;
    }
    buf.push((val & 0x7F) as u8);
}

pub fn encode_varint_field(buf: &mut Vec<u8>, field_no: u32, val: u64) {
    let tag = ((field_no as u64) << 3) | 0;
    encode_varint(tag, buf);
    encode_varint(val, buf);
}

pub fn encode_bytes_field(buf: &mut Vec<u8>, field_no: u32, bytes: &[u8]) {
    let tag = ((field_no as u64) << 3) | 2;
    encode_varint(tag, buf);
    encode_varint(bytes.len() as u64, buf);
    buf.extend_from_slice(bytes);
}

pub fn encode_ump_chunk(part_type: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_varint(part_type, &mut out);
    encode_varint(payload.len() as u64, &mut out);
    out.extend_from_slice(payload);
    out
}

pub fn parse_media_header_itag(payload: &[u8]) -> Option<u32> {
    let mut reader = ProtoReader::new(payload);
    while !reader.data.is_empty() {
        if let Ok((field_no, wire_type)) = reader.read_tag() {
            if field_no == 2 && wire_type == 0 {
                if let Ok(itag) = reader.read_varint() {
                    return Some(itag as u32);
                }
            } else {
                let _ = reader.skip_field(wire_type);
            }
        } else {
            break;
        }
    }
    None
}

pub fn build_sabr_request_body(session: &NormalizedSabrSession) -> Vec<u8> {
    if let Some(ref rb) = session.request_body {
        if let Ok(bytes) = BASE64_STANDARD.decode(rb.as_bytes()) {
            if !bytes.is_empty() {
                return bytes;
            }
        }
    }
    let mut body = Vec::new();
    encode_varint_field(&mut body, 3, session.audio_itag as u64);
    if let Some(video_itag) = session.video_itag {
        encode_varint_field(&mut body, 4, video_itag as u64);
    }
    let mut streamer_ctx = Vec::new();
    let mut client_bytes = Vec::new();
    encode_varint_field(&mut client_bytes, 1, session.client_info.client_name as u64);
    if !session.client_info.client_version.is_empty() {
        encode_bytes_field(&mut client_bytes, 2, session.client_info.client_version.as_bytes());
    }
    encode_bytes_field(&mut streamer_ctx, 6, &client_bytes);

    if let Some(ref po_token_b64) = session.po_token {
        if let Ok(po_bytes) = BASE64_STANDARD.decode(po_token_b64.as_bytes()) {
            encode_bytes_field(&mut streamer_ctx, 19, &po_bytes);
        }
    }
    encode_bytes_field(&mut body, 10, &streamer_ctx);
    body
}

pub fn parse_sabr_protobuf_request(bytes: &[u8]) -> Result<ParsedSabrSeed, String> {
    let mut reader = ProtoReader::new(bytes);
    let mut streamer_context_bytes: Option<&[u8]> = None;

    while !reader.data.is_empty() {
        let (field_no, wire_type) = reader.read_tag()?;
        if field_no == 10 && wire_type == 2 {
            streamer_context_bytes = Some(reader.read_length_delimited()?);
        } else {
            reader.skip_field(wire_type)?;
        }
    }

    let mut seed = ParsedSabrSeed::default();

    if let Some(ctx_bytes) = streamer_context_bytes {
        let mut ctx_reader = ProtoReader::new(ctx_bytes);
        while !ctx_reader.data.is_empty() {
            let (field_no, wire_type) = ctx_reader.read_tag()?;
            match (field_no, wire_type) {
                (6, 2) => {
                    let client_bytes = ctx_reader.read_length_delimited()?;
                    seed.client_info = Some(parse_client_info(client_bytes)?);
                }
                (19, 2) => {
                    let po_bytes = ctx_reader.read_length_delimited()?;
                    if !po_bytes.is_empty() {
                        seed.po_token = Some(po_bytes.to_vec());
                    }
                }
                (_, wt) => {
                    ctx_reader.skip_field(wt)?;
                }
            }
        }
    }

    Ok(seed)
}

fn parse_client_info(bytes: &[u8]) -> Result<SabrClientInfo, String> {
    let mut reader = ProtoReader::new(bytes);
    let mut info = SabrClientInfo::default();

    while !reader.data.is_empty() {
        let (field_no, wire_type) = reader.read_tag()?;
        match (field_no, wire_type) {
            (1, 0) => {
                info.client_name = reader.read_varint()? as u32;
            }
            (1, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                if let Ok(s) = std::str::from_utf8(str_bytes) {
                    if let Ok(num) = s.parse::<u32>() {
                        info.client_name = num;
                    }
                }
            }
            (2, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                info.client_version = String::from_utf8_lossy(str_bytes).to_string();
            }
            (3, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                info.hl = Some(String::from_utf8_lossy(str_bytes).to_string());
            }
            (4, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                info.device_make = Some(String::from_utf8_lossy(str_bytes).to_string());
            }
            (5, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                info.device_model = Some(String::from_utf8_lossy(str_bytes).to_string());
            }
            (6, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                info.os_name = Some(String::from_utf8_lossy(str_bytes).to_string());
            }
            (7, 2) => {
                let str_bytes = reader.read_length_delimited()?;
                info.os_version = Some(String::from_utf8_lossy(str_bytes).to_string());
            }
            (_, wt) => {
                reader.skip_field(wt)?;
            }
        }
    }

    Ok(info)
}

pub fn normalize_sabr_session(value: &SabrSession) -> Result<NormalizedSabrSession, String> {
    if value.video_id.len() != 11
        || !value
            .video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid YouTube SABR session.".to_owned());
    }

    let server_abr_streaming_url = google_video_sabr_url(&value.server_abr_streaming_url)?;

    if value.video_playback_ustreamer_config.is_empty()
        || value.video_playback_ustreamer_config.len() > 131_072
    {
        return Err("Missing YouTube streaming configuration.".to_owned());
    }

    if value.formats.is_empty() || value.formats.len() > 150 {
        return Err("Missing YouTube SABR formats.".to_owned());
    }

    let audio_itag = value.audio_itag;
    let video_itag = value.video_itag;

    let audio_candidates: Vec<&SabrFormat> = value
        .formats
        .iter()
        .filter(|f| {
            f.itag == audio_itag
                && f.mime_type.to_ascii_lowercase().starts_with("audio/mp4")
                && !f.is_drc
                && !f.is_vb
        })
        .collect();

    let audio_raw = audio_candidates
        .iter()
        .find(|f| f.audio_track.as_ref().map_or(false, |t| t.audio_is_default))
        .or_else(|| audio_candidates.iter().find(|f| f.audio_track.is_none()))
        .or_else(|| audio_candidates.first())
        .copied();

    let video_raw = video_itag.and_then(|v_itag| {
        value
            .formats
            .iter()
            .find(|f| f.itag == v_itag && f.mime_type.to_ascii_lowercase().starts_with("video/mp4"))
    });

    let mut chosen_formats = Vec::new();
    if let Some(v) = video_raw {
        let mut fmt = v.clone();
        fmt.has_audio = fmt.mime_type.to_ascii_lowercase().starts_with("audio/");
        fmt.has_video = true;
        chosen_formats.push(fmt);
    }
    if let Some(a) = audio_raw {
        let mut fmt = a.clone();
        fmt.has_audio = true;
        fmt.has_video = fmt.mime_type.to_ascii_lowercase().starts_with("video/");
        fmt.audio_track_id = fmt.audio_track.as_ref().and_then(|t| t.id.clone());
        chosen_formats.push(fmt);
    }

    if !chosen_formats
        .iter()
        .any(|f| f.itag == audio_itag && f.has_audio)
    {
        return Err("The selected YouTube audio track is unavailable.".to_owned());
    }

    if let Some(v_itag) = video_itag {
        if !chosen_formats
            .iter()
            .any(|f| f.itag == v_itag && f.has_video)
        {
            return Err("The selected YouTube video track is unavailable.".to_owned());
        }
    }

    let mut seed: Option<ParsedSabrSeed> = None;
    if let Some(ref rb) = value.request_body {
        if rb.is_empty() || rb.len() > 350_000 || !is_valid_base64_str(rb) {
            return Err("Invalid YouTube SABR request context.".to_owned());
        }
        let decoded = BASE64_STANDARD
            .decode(rb.as_bytes())
            .map_err(|_| "Invalid YouTube SABR request context.".to_owned())?;
        if decoded.is_empty() || decoded.len() > 262_144 {
            return Err("Invalid YouTube SABR request context.".to_owned());
        }
        let parsed = parse_sabr_protobuf_request(&decoded)
            .map_err(|_| "The captured YouTube SABR request context is unreadable.".to_owned())?;
        seed = Some(parsed);
    }

    let captured_client = seed.as_ref().and_then(|s| s.client_info.as_ref());
    let incoming_client = value.client_info.as_ref();

    let client_name = captured_client
        .map(|c| c.client_name)
        .or_else(|| incoming_client.map(|c| c.client_name))
        .unwrap_or(1);

    let client_version = captured_client
        .filter(|c| !c.client_version.is_empty())
        .map(|c| c.client_version.clone())
        .or_else(|| incoming_client.map(|c| c.client_version.clone()))
        .unwrap_or_default();

    let hl = captured_client
        .and_then(|c| c.hl.clone())
        .or_else(|| incoming_client.and_then(|c| c.hl.clone()));
    let device_make = captured_client
        .and_then(|c| c.device_make.clone())
        .or_else(|| incoming_client.and_then(|c| c.device_make.clone()));
    let device_model = captured_client
        .and_then(|c| c.device_model.clone())
        .or_else(|| incoming_client.and_then(|c| c.device_model.clone()));
    let os_name = captured_client
        .and_then(|c| c.os_name.clone())
        .or_else(|| incoming_client.and_then(|c| c.os_name.clone()));
    let os_version = captured_client
        .and_then(|c| c.os_version.clone())
        .or_else(|| incoming_client.and_then(|c| c.os_version.clone()));

    let client_info = SabrClientInfo {
        client_name,
        client_version,
        hl,
        device_make,
        device_model,
        os_name,
        os_version,
    };

    let po_token = seed
        .as_ref()
        .and_then(|s| s.po_token.as_ref())
        .map(|bytes| BASE64_STANDARD.encode(bytes))
        .or_else(|| value.po_token.clone());

    Ok(NormalizedSabrSession {
        video_id: value.video_id.clone(),
        server_abr_streaming_url,
        video_playback_ustreamer_config: value.video_playback_ustreamer_config.clone(),
        client_info,
        formats: chosen_formats,
        video_itag,
        audio_itag,
        request_body: value.request_body.clone(),
        po_token,
    })
}

fn is_valid_base64_str(value: &str) -> bool {
    let mut padding = 0;
    for c in value.chars() {
        if padding > 0 {
            if c != '=' || padding > 2 {
                return false;
            }
            padding += 1;
        } else if c == '=' {
            padding = 1;
        } else if !c.is_ascii_alphanumeric() && c != '+' && c != '/' {
            return false;
        }
    }
    true
}

fn bounded_integer(val: Option<&serde_json::Value>, max: u64) -> Result<u64, String> {
    let num = val
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "Invalid SABR number.".to_owned())?;
    if num > max {
        return Err("Invalid SABR number.".to_owned());
    }
    Ok(num)
}

fn bounded_text(value: &str, max: usize, pattern: fn(char) -> bool) -> Result<String, String> {
    if value.len() > max || !value.chars().all(pattern) {
        return Err("Invalid SABR text.".to_owned());
    }
    Ok(value.to_owned())
}

pub fn sanitize_sabr(
    value: &serde_json::Value,
    download_url: &str,
) -> Result<SabrSession, String> {
    if !value.is_object() {
        return Err("Invalid SABR session.".to_owned());
    }

    let raw_streaming_url = value
        .get("serverAbrStreamingUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid SABR session.".to_owned())?;

    let stream = Url::parse(raw_streaming_url).map_err(|_| "Invalid SABR stream URL.".to_owned())?;
    if stream.as_str() != download_url {
        return Err("Invalid SABR stream URL.".to_owned());
    }

    let host = stream.host_str().unwrap_or("").to_ascii_lowercase();
    if host != "googlevideo.com" && !host.ends_with(".googlevideo.com") {
        return Err("Invalid SABR stream URL.".to_owned());
    }
    if stream.path() != "/videoplayback" {
        return Err("Invalid SABR stream URL.".to_owned());
    }
    if stream.query_pairs().find(|(k, v)| k == "sabr" && v == "1").is_none() {
        return Err("Invalid SABR stream URL.".to_owned());
    }

    let raw_video_id = value
        .get("videoId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid SABR text.".to_owned())?;
    if raw_video_id.len() != 11
        || !raw_video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid SABR text.".to_owned());
    }

    let raw_ustreamer = value
        .get("videoPlaybackUstreamerConfig")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid SABR text.".to_owned())?;
    let ustreamer_config = bounded_text(raw_ustreamer, 131_072, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '/' | '=' | '.' | '-' | ':' | ';' | ',' | '~')
    })?;

    let raw_formats = value
        .get("formats")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid SABR formats.".to_owned())?;
    if raw_formats.is_empty() || raw_formats.len() > 150 {
        return Err("Invalid SABR formats.".to_owned());
    }

    let mut formats = Vec::with_capacity(raw_formats.len());
    for raw in raw_formats {
        if !raw.is_object() {
            return Err("Invalid SABR format.".to_owned());
        }

        let itag = bounded_integer(raw.get("itag"), 99_999)? as u32;

        let mime = raw
            .get("mimeType")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Invalid SABR text.".to_owned())?;
        if mime.len() > 200 {
            return Err("Invalid SABR text.".to_owned());
        }
        let lower_mime = mime.to_ascii_lowercase();
        let valid_mime_prefix = lower_mime.starts_with("audio/mp4")
            || lower_mime.starts_with("video/mp4")
            || lower_mime.starts_with("audio/webm")
            || lower_mime.starts_with("video/webm");
        if !valid_mime_prefix || mime.contains('\r') || mime.contains('\n') || mime.contains('\0') {
            return Err("Invalid SABR text.".to_owned());
        }

        let bitrate = raw
            .get("bitrate")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let width = if let Some(v) = raw.get("width") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let height = if let Some(v) = raw.get("height") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let fps = if let Some(v) = raw.get("fps") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let average_bitrate = if let Some(v) = raw.get("averageBitrate") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let audio_channels = if let Some(v) = raw.get("audioChannels") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };

        let last_modified = if let Some(v) = raw.get("lastModified") {
            let s = v.as_str().or_else(|| v.as_u64().map(|_| "")).unwrap_or("");
            let val_str = if s.is_empty() && v.is_number() {
                v.to_string()
            } else {
                s.to_string()
            };
            Some(bounded_text(&val_str, 32, |c| c.is_ascii_digit())?)
        } else {
            None
        };

        let content_length = if let Some(v) = raw.get("contentLength") {
            let s = v.as_str().or_else(|| v.as_u64().map(|_| "")).unwrap_or("");
            let val_str = if s.is_empty() && v.is_number() {
                v.to_string()
            } else {
                s.to_string()
            };
            Some(bounded_text(&val_str, 32, |c| c.is_ascii_digit())?)
        } else {
            None
        };

        let approx_duration_ms = if let Some(v) = raw.get("approxDurationMs") {
            let s = v.as_str().or_else(|| v.as_u64().map(|_| "")).unwrap_or("");
            let val_str = if s.is_empty() && v.is_number() {
                v.to_string()
            } else {
                s.to_string()
            };
            Some(bounded_text(&val_str, 32, |c| c.is_ascii_digit())?)
        } else {
            None
        };

        let target_duration_sec = if let Some(v) = raw.get("targetDurationSec") {
            let n = v.as_f64().ok_or_else(|| "Invalid SABR duration.".to_owned())?;
            if !n.is_finite() || n < 0.0 || n > 86_400.0 {
                return Err("Invalid SABR duration.".to_owned());
            }
            Some(n)
        } else {
            None
        };

        let xtags = if let Some(v) = raw.get("xtags").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 2048, |c| c != '\r' && c != '\n' && c != '\0')?)
        } else {
            None
        };

        let audio_quality = if let Some(v) = raw.get("audioQuality").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')?)
        } else {
            None
        };

        let quality = if let Some(v) = raw.get("quality").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')' | '/' | '+' | ','))?)
        } else {
            None
        };

        let quality_label = if let Some(v) = raw.get("qualityLabel").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')' | '/' | '+' | ','))?)
        } else {
            None
        };

        let audio_track = if let Some(at) = raw.get("audioTrack").and_then(|v| v.as_object()) {
            let id = if let Some(id_val) = at.get("id").and_then(|v| v.as_str()) {
                Some(bounded_text(id_val, 100, |c| c != '\r' && c != '\n' && c != '\0')?)
            } else {
                None
            };
            let audio_is_default = at.get("audioIsDefault").and_then(|v| v.as_bool()).unwrap_or(false);
            Some(SabrAudioTrack {
                id,
                audio_is_default,
            })
        } else {
            None
        };

        let color_info = if let Some(ci) = raw.get("colorInfo").and_then(|v| v.as_object()) {
            let tc = if let Some(s) = ci.get("transferCharacteristics").and_then(|v| v.as_str()) {
                Some(bounded_text(s, 80, |c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '))?)
            } else {
                None
            };
            Some(SabrColorInfo {
                transfer_characteristics: tc,
            })
        } else {
            None
        };

        let is_drc = raw.get("isDrc").and_then(|v| v.as_bool()).unwrap_or(false);
        let is_vb = raw.get("isVb").and_then(|v| v.as_bool()).unwrap_or(false);

        formats.push(SabrFormat {
            itag,
            mime_type: mime.to_owned(),
            bitrate,
            width,
            height,
            fps,
            average_bitrate,
            audio_channels,
            last_modified,
            content_length,
            approx_duration_ms,
            target_duration_sec,
            xtags,
            audio_quality,
            quality,
            quality_label,
            audio_track,
            color_info,
            is_drc,
            is_vb,
            has_audio: false,
            has_video: false,
            audio_track_id: None,
        });
    }

    let audio_itag = bounded_integer(value.get("audioItag"), 99_999)? as u32;
    let video_itag = if let Some(v) = value.get("videoItag") {
        Some(bounded_integer(Some(v), 99_999)? as u32)
    } else {
        None
    };

    let has_audio_format = formats
        .iter()
        .any(|f| f.itag == audio_itag && f.mime_type.to_ascii_lowercase().starts_with("audio/mp4"));
    if !has_audio_format {
        return Err("Selected SABR format is unavailable.".to_owned());
    }

    if let Some(v_itag) = video_itag {
        let has_video_format = formats
            .iter()
            .any(|f| f.itag == v_itag && f.mime_type.to_ascii_lowercase().starts_with("video/mp4"));
        if !has_video_format {
            return Err("Selected SABR format is unavailable.".to_owned());
        }
    }

    let client = value.get("clientInfo").and_then(|v| v.as_object());
    let client_name = if let Some(c) = client {
        bounded_integer(c.get("clientName"), 10_000).unwrap_or(1) as u32
    } else {
        1
    };
    let client_version = if let Some(c) = client {
        if let Some(v) = c.get("clientVersion").and_then(|v| v.as_str()) {
            bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')?
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let client_info = SabrClientInfo {
        client_name,
        client_version,
        hl: None,
        device_make: None,
        device_model: None,
        os_name: None,
        os_version: None,
    };

    let request_body = if let Some(v) = value.get("requestBody").and_then(|v| v.as_str()) {
        let text = bounded_text(v, 350_000, |c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')?;
        let decoded = BASE64_STANDARD
            .decode(text.as_bytes())
            .map_err(|_| "Invalid SABR request body.".to_owned())?;
        if decoded.is_empty() || decoded.len() > 262_144 {
            return Err("Invalid SABR request body.".to_owned());
        }
        let re_encoded = BASE64_STANDARD.encode(&decoded);
        if re_encoded.trim_end_matches('=') != text.trim_end_matches('=') {
            return Err("Invalid SABR request body.".to_owned());
        }
        Some(text)
    } else {
        None
    };

    let session = SabrSession {
        video_id: raw_video_id.to_owned(),
        server_abr_streaming_url: raw_streaming_url.to_owned(),
        video_playback_ustreamer_config: ustreamer_config,
        client_info: Some(client_info),
        formats,
        video_itag,
        audio_itag,
        request_body,
        po_token: None,
    };

    Ok(session)
}
