// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: Server-Adaptive BitRate (SABR) YouTube protocol definitions and stream decoders.

pub mod proto;
pub mod request;
pub mod sanitize;
pub mod types;

pub use proto::{
    encode_bytes_field, encode_ump_chunk, encode_varint, encode_varint_field,
    parse_media_header_itag, ProtoReader,
};
pub use request::{build_sabr_request_body, parse_sabr_protobuf_request};
pub use sanitize::{google_video_sabr_url, normalize_sabr_session, sanitize_sabr};
pub use types::{
    NormalizedSabrSession, ParsedSabrSeed, SabrAudioTrack, SabrClientInfo, SabrColorInfo,
    SabrFormat, SabrSession,
};
