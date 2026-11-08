// Created: 2026-11-05
// Reason: feat: add segmented transfers, pause/resume, scheduling, and rate limiting
// Last updated: 2026-11-07
// Reason: feat(media): complete YouTube media pipeline, desktop quality selector, resilient download resumption, and native messaging registration
// Description: Token bucket rate limiter enforcing bandwidth caps across active download transfers.

use std::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::{Duration, Instant},
};

use tokio::time::sleep_until;

pub struct DownloadRateLimiter {
    started_at: Instant,
    next_allowed_nanos: AtomicU64,
    limit_kbps: AtomicU32,
}

impl DownloadRateLimiter {
    pub fn new(limit_kbps: u32) -> Self {
        Self {
            started_at: Instant::now(),
            next_allowed_nanos: AtomicU64::new(0),
            limit_kbps: AtomicU32::new(limit_kbps),
        }
    }

    pub fn limit_kbps(&self) -> u32 {
        self.limit_kbps.load(Ordering::Acquire)
    }

    pub fn set_limit_kbps(&self, limit_kbps: u32) {
        self.limit_kbps.store(limit_kbps, Ordering::Release);
        self.next_allowed_nanos.store(
            self.started_at
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
            Ordering::Release,
        );
    }

    pub async fn throttle(&self, bytes: u64) {
        let limit_kbps = self.limit_kbps.load(Ordering::Acquire);
        if limit_kbps == 0 || bytes == 0 {
            return;
        }

        let now = self
            .started_at
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        let duration_nanos = u128::from(bytes)
            .saturating_mul(1_000_000_000)
            .div_ceil(u128::from(limit_kbps) * 1024)
            .min(u128::from(u64::MAX)) as u64;
        let mut current = self.next_allowed_nanos.load(Ordering::Acquire);
        let deadline = loop {
            let deadline = current.max(now).saturating_add(duration_nanos);
            match self.next_allowed_nanos.compare_exchange_weak(
                current,
                deadline,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break deadline,
                Err(updated) => current = updated,
            }
        };
        sleep_until(tokio::time::Instant::from_std(
            self.started_at + Duration::from_nanos(deadline),
        ))
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::DownloadRateLimiter;

    #[tokio::test]
    async fn concurrent_transfers_share_the_configured_bandwidth_limit() {
        let limiter = std::sync::Arc::new(DownloadRateLimiter::new(64));
        let started = Instant::now();
        let first = {
            let limiter = limiter.clone();
            tokio::spawn(async move {
                limiter.throttle(32 * 1024).await;
            })
        };
        let second = {
            let limiter = limiter.clone();
            tokio::spawn(async move {
                limiter.throttle(32 * 1024).await;
            })
        };
        first
            .await
            .expect("first transfer should finish throttling");
        second
            .await
            .expect("second transfer should finish throttling");

        assert!(
            started.elapsed() >= Duration::from_millis(900),
            "two 32 KiB transfers should share one 64 KiB/s limit"
        );
    }

    #[tokio::test]
    async fn zero_limit_does_not_delay_transfers() {
        let limiter = DownloadRateLimiter::new(0);
        let started = Instant::now();
        limiter.throttle(1024 * 1024).await;
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}
