use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TransferMetrics {
    pub download_id: String,
    pub transfer_kind: String,
    pub created_at: Instant,
    pub first_request_at: Option<Instant>,
    pub first_response_at: Option<Instant>,
    pub first_byte_at: Option<Instant>,
    pub completed_at: Option<Instant>,
    pub requested_concurrency: u8,
    pub effective_concurrency: u8,
    pub range_supported: bool,
    pub known_total: Option<u64>,
    pub retries: u32,
    pub fallback_reason: Option<String>,
}

impl TransferMetrics {
    pub fn new(download_id: impl Into<String>, transfer_kind: impl Into<String>, requested_concurrency: u8) -> Self {
        Self {
            download_id: download_id.into(),
            transfer_kind: transfer_kind.into(),
            created_at: Instant::now(),
            first_request_at: None,
            first_response_at: None,
            first_byte_at: None,
            completed_at: None,
            requested_concurrency,
            effective_concurrency: requested_concurrency.max(1),
            range_supported: false,
            known_total: None,
            retries: 0,
            fallback_reason: None,
        }
    }

    pub fn record_first_request(&mut self) {
        if self.first_request_at.is_none() {
            self.first_request_at = Some(Instant::now());
        }
    }

    pub fn record_first_response(&mut self) {
        if self.first_response_at.is_none() {
            self.first_response_at = Some(Instant::now());
        }
    }

    pub fn record_first_byte(&mut self) {
        if self.first_byte_at.is_none() {
            let now = Instant::now();
            self.first_byte_at = Some(now);
            let ttfb = now.duration_since(self.created_at);
            eprintln!(
                "[metrics] first_byte download_id={} kind={} ttfb_ms={}",
                self.download_id,
                self.transfer_kind,
                ttfb.as_millis()
            );
        }
    }

    pub fn record_retry(&mut self) {
        self.retries += 1;
    }

    pub fn set_fallback(&mut self, reason: impl Into<String>) {
        self.fallback_reason = Some(reason.into());
    }

    pub fn record_completion(&mut self, total_bytes: u64) {
        let now = Instant::now();
        self.completed_at = Some(now);
        let elapsed = now.duration_since(self.created_at);
        let ttfb_ms = self.first_byte_at.map(|t| t.duration_since(self.created_at).as_millis());
        let avg_speed_kbps = if elapsed.as_millis() > 0 {
            (total_bytes as f64 / elapsed.as_secs_f64()) / 1024.0
        } else {
            0.0
        };

        eprintln!(
            "[metrics] complete download_id={} kind={} req_concurrency={} eff_concurrency={} range={} total_bytes={} elapsed_ms={} ttfb_ms={:?} avg_speed_kbps={} retries={} fallback={:?}",
            self.download_id,
            self.transfer_kind,
            self.requested_concurrency,
            self.effective_concurrency,
            self.range_supported,
            total_bytes,
            elapsed.as_millis(),
            ttfb_ms,
            avg_speed_kbps as u64,
            self.retries,
            self.fallback_reason
        );
    }
}
