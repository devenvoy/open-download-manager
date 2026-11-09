// Created: 2026-11-08
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Last updated: 2026-11-10
// Reason: refactor: modularize MMS domain protocol into variant, session, and asx parser submodules
// Description: Domain protocol detector, session state machine, and safe bounded ASX/WVX playlist parser for MMS.

pub mod asx;
pub mod session;
pub mod variant;

pub use asx::{AsxEntry, AsxPlaylist, AsxPlaylistParser, MAX_ASX_ENTRIES, MAX_ASX_PLAYLIST_BYTES};
pub use session::{
    MmsSessionState, MmsSessionStateMachine, MMS_STAGE_CONNECTING, MMS_STAGE_HANDSHAKING,
};
pub use variant::{MmsProtocolDetector, MmsVariant};
