use std::time::{Duration, Instant};

/// `ThroughputMeter` calculates real-time download throughput using a moving
/// sampling window (default 250ms).
///
/// It avoids floating-point inaccuracies, handles clock jitter gracefully,
/// and prevents division-by-zero or timestamp rollover issues.
#[derive(Debug, Clone)]
pub struct ThroughputMeter {
    last_sample_time: Instant,
    last_sample_bytes: u64,
    current_throughput_bps: u64,
    sample_interval: Duration,
}

impl Default for ThroughputMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroughputMeter {
    pub const DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

    pub fn new() -> Self {
        Self::with_interval(Self::DEFAULT_SAMPLE_INTERVAL)
    }

    pub fn with_interval(interval: Duration) -> Self {
        Self {
            last_sample_time: Instant::now(),
            last_sample_bytes: 0,
            current_throughput_bps: 0,
            sample_interval: interval,
        }
    }

    pub fn with_initial_bytes(initial_bytes: u64) -> Self {
        Self {
            last_sample_time: Instant::now(),
            last_sample_bytes: initial_bytes,
            current_throughput_bps: 0,
            sample_interval: Self::DEFAULT_SAMPLE_INTERVAL,
        }
    }

    /// Records total bytes transferred and recalculates current throughput if
    /// the sampling window has elapsed.
    ///
    /// Returns `Some(current_speed_bps)` if throughput was updated during this tick,
    /// or `None` if within the sampling interval.
    pub fn update(&mut self, total_bytes: u64) -> Option<u64> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_sample_time);
        if elapsed >= self.sample_interval {
            let elapsed_millis = elapsed.as_millis().max(1);
            let byte_delta = total_bytes.saturating_sub(self.last_sample_bytes);
            self.current_throughput_bps = (byte_delta * 1000) / elapsed_millis as u64;
            self.last_sample_bytes = total_bytes;
            self.last_sample_time = now;
            Some(self.current_throughput_bps)
        } else {
            None
        }
    }

    /// Forces an immediate throughput recalculation regardless of whether the sampling interval elapsed.
    pub fn force_update(&mut self, total_bytes: u64) -> u64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_sample_time);
        let elapsed_millis = elapsed.as_millis().max(1);
        let byte_delta = total_bytes.saturating_sub(self.last_sample_bytes);
        self.current_throughput_bps = (byte_delta * 1000) / elapsed_millis as u64;
        self.last_sample_bytes = total_bytes;
        self.last_sample_time = now;
        self.current_throughput_bps
    }

    pub fn current_throughput(&self) -> u64 {
        self.current_throughput_bps
    }

    pub fn reset(&mut self, current_bytes: u64) {
        self.last_sample_time = Instant::now();
        self.last_sample_bytes = current_bytes;
        self.current_throughput_bps = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_meter_starts_at_zero() {
        let meter = ThroughputMeter::new();
        assert_eq!(meter.current_throughput(), 0);
    }

    #[test]
    fn updates_within_interval_return_none() {
        let mut meter = ThroughputMeter::with_interval(Duration::from_secs(10));
        assert!(meter.update(1024).is_none());
        assert_eq!(meter.current_throughput(), 0);
    }

    #[test]
    fn force_update_computes_speed() {
        let mut meter = ThroughputMeter::new();
        std::thread::sleep(Duration::from_millis(50));
        let speed = meter.force_update(50_000);
        assert!(speed > 0);
        assert_eq!(meter.current_throughput(), speed);
    }

    #[test]
    fn reset_clears_speed_and_adjusts_baseline() {
        let mut meter = ThroughputMeter::new();
        std::thread::sleep(Duration::from_millis(50));
        meter.force_update(100_000);
        assert!(meter.current_throughput() > 0);

        meter.reset(100_000);
        assert_eq!(meter.current_throughput(), 0);
    }
}
