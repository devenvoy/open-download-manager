// Created: 2026-11-10
// Reason: refactor: modularize MMS domain protocol into variant, session, and asx parser submodules
// Description: MMS connection and session state machine with diagnostic markers.

use serde::{Deserialize, Serialize};

/// Diagnostic marker strings reflecting the MMS transport stages.
pub const MMS_STAGE_CONNECTING: &str = "MMSS:TrStNCn";
pub const MMS_STAGE_HANDSHAKING: &str = "MMSSss:SndAlCnInf";

/// Explicit connection and session states for the MMS streaming lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "detail")]
pub enum MmsSessionState {
    /// Session created, not yet connected.
    Initial,
    /// Transport connection starting (MMSS:TrStNCn).
    Connecting { endpoint: String },
    /// Session handshake and connection info negotiation (MMSSss:SndAlCnInf).
    Handshaking { stream_offset: u64 },
    /// Active media chunk streaming.
    Streaming {
        bytes_received: u64,
        chunks_processed: u64,
    },
    /// Stream completed cleanly (ASF $E chunk or EOF reached).
    Completed { total_bytes: u64 },
    /// Transfer explicitly cancelled by user.
    Cancelled,
    /// Midstream disconnection; eligible for bounded reconnect.
    Disconnected {
        bytes_at_disconnect: u64,
        reason: String,
    },
    /// Fatal session failure.
    Failed { error: String },
}

/// Dedicated MMS session state machine enforcing state boundaries and diagnostic tracking.
#[derive(Clone, Debug)]
pub struct MmsSessionStateMachine {
    state: MmsSessionState,
    diagnostic_events: Vec<String>,
}

impl Default for MmsSessionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl MmsSessionStateMachine {
    /// Creates a new MMS state machine in the Initial state.
    pub fn new() -> Self {
        Self {
            state: MmsSessionState::Initial,
            diagnostic_events: vec!["MMS:Initial".to_string()],
        }
    }

    /// Transitions to the Connecting state (`MMSS:TrStNCn`).
    pub fn start_connection(&mut self, endpoint: &str) {
        self.diagnostic_events
            .push(format!("{MMS_STAGE_CONNECTING}:{endpoint}"));
        self.state = MmsSessionState::Connecting {
            endpoint: endpoint.to_string(),
        };
    }

    /// Transitions to the Handshaking state (`MMSSss:SndAlCnInf`).
    pub fn send_connection_info(&mut self, stream_offset: u64) {
        self.diagnostic_events
            .push(format!("{MMS_STAGE_HANDSHAKING}:offset={stream_offset}"));
        self.state = MmsSessionState::Handshaking { stream_offset };
    }

    /// Transitions to the active Streaming state.
    pub fn begin_streaming(&mut self) {
        self.diagnostic_events.push("MMS:StreamingStarted".to_string());
        self.state = MmsSessionState::Streaming {
            bytes_received: 0,
            chunks_processed: 0,
        };
    }

    /// Updates byte counts during chunk processing.
    pub fn record_chunk(&mut self, chunk_bytes: usize) {
        if let MmsSessionState::Streaming {
            ref mut bytes_received,
            ref mut chunks_processed,
        } = self.state
        {
            *bytes_received += chunk_bytes as u64;
            *chunks_processed += 1;
        }
    }

    /// Transitions to the Completed state.
    pub fn complete(&mut self, total_bytes: u64) {
        self.diagnostic_events
            .push(format!("MMS:Completed:total={total_bytes}"));
        self.state = MmsSessionState::Completed { total_bytes };
    }

    /// Transitions to the Disconnected state for reconnect evaluation.
    pub fn disconnect(&mut self, reason: &str) {
        let bytes = match &self.state {
            MmsSessionState::Streaming { bytes_received, .. } => *bytes_received,
            _ => 0,
        };
        self.diagnostic_events
            .push(format!("MMS:Disconnected:{reason}:bytes={bytes}"));
        self.state = MmsSessionState::Disconnected {
            bytes_at_disconnect: bytes,
            reason: reason.to_string(),
        };
    }

    /// Transitions to the Cancelled state.
    pub fn cancel(&mut self) {
        self.diagnostic_events.push("MMS:Cancelled".to_string());
        self.state = MmsSessionState::Cancelled;
    }

    /// Transitions to the Failed state.
    pub fn fail(&mut self, error: &str) {
        self.diagnostic_events
            .push(format!("MMS:Failed:{error}"));
        self.state = MmsSessionState::Failed {
            error: error.to_string(),
        };
    }

    /// Returns the current session state.
    pub fn state(&self) -> &MmsSessionState {
        &self.state
    }

    /// Returns recorded diagnostic trace events.
    pub fn diagnostic_events(&self) -> &[String] {
        &self.diagnostic_events
    }
}
