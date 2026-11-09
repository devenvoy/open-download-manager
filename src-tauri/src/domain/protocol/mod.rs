// Created: 2026-11-08
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: Domain protocol boundaries, detection policies, and media playlist resolvers.

pub mod mms;

pub use mms::{
    AsxEntry, AsxPlaylist, AsxPlaylistParser, MmsProtocolDetector, MmsSessionState,
    MmsSessionStateMachine, MmsVariant, MAX_ASX_ENTRIES, MAX_ASX_PLAYLIST_BYTES,
    MMS_STAGE_CONNECTING, MMS_STAGE_HANDSHAKING,
};
