use base64::prelude::*;

use super::{
    proto::{encode_bytes_field, encode_varint_field, ProtoReader},
    types::{NormalizedSabrSession, ParsedSabrSeed, SabrClientInfo},
};

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
