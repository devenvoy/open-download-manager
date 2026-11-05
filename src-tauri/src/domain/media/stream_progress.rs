use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamProgress {
    pub completed: usize,
    pub total: usize,
    pub estimated_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSegment {
    pub done: bool,
    pub downloaded: u64,
}

pub fn calculate_stream_progress(segments: &[StreamSegment]) -> StreamProgress {
    let completed = segments.iter().filter(|s| s.done).count();
    let total = segments.len();
    let downloaded: u64 = segments.iter().map(|s| s.downloaded).sum();
    let completed_bytes: u64 = segments.iter().filter(|s| s.done).map(|s| s.downloaded).sum();
    let estimated_bytes = if completed >= 2 && completed < total {
        let avg_per_segment = completed_bytes / (completed as u64);
        let estimated = avg_per_segment * (total as u64);
        Some(estimated.max(downloaded))
    } else {
        None
    };
    StreamProgress {
        completed,
        total,
        estimated_bytes,
    }
}
