// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(engine): persisted binary range map for crash-safe sparse file resume (W3)

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const RANGE_MAP_MAGIC: [u8; 4] = [0x4F, 0x44, 0x4D, 0x52]; // "ODMR"
pub const RANGE_MAP_VERSION: u32 = 1;

/// Deterministic FNV-1a 64-bit hash for ETag strings.
pub fn hash_etag(etag: Option<&str>) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    if let Some(s) = etag {
        for byte in s.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Helper to get the .odm.ranges sidecar path for a partial download file.
pub fn range_map_path(part_path: &Path) -> PathBuf {
    part_path.with_extension("ranges")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeMap {
    pub total: u64,
    pub etag_hash: u64,
    pub ranges: Vec<(u64, u64)>, // Sorted, non-overlapping inclusive intervals [start, end]
}

impl RangeMap {
    pub fn new(total: u64, etag_hash: u64) -> Self {
        Self {
            total,
            etag_hash,
            ranges: Vec::new(),
        }
    }

    /// Insert a newly downloaded byte range [start, end] and merge adjacent/overlapping ranges.
    pub fn add_range(&mut self, start: u64, end: u64) {
        if start > end || start >= self.total {
            return;
        }
        let clamped_end = end.min(self.total.saturating_sub(1));
        let mut new_ranges = Vec::with_capacity(self.ranges.len() + 1);
        let mut inserted_start = start;
        let mut inserted_end = clamped_end;
        let mut inserted = false;

        for &(r_start, r_end) in &self.ranges {
            if inserted {
                new_ranges.push((r_start, r_end));
            } else if r_end + 1 < inserted_start {
                // Range is entirely before the inserted range
                new_ranges.push((r_start, r_end));
            } else if r_start > inserted_end + 1 {
                // Range is entirely after the inserted range
                new_ranges.push((inserted_start, inserted_end));
                new_ranges.push((r_start, r_end));
                inserted = true;
            } else {
                // Overlaps or is adjacent -> merge
                inserted_start = inserted_start.min(r_start);
                inserted_end = inserted_end.max(r_end);
            }
        }

        if !inserted {
            new_ranges.push((inserted_start, inserted_end));
        }

        self.ranges = new_ranges;
    }

    /// Calculate total downloaded bytes across all ranges.
    pub fn downloaded_bytes(&self) -> u64 {
        self.ranges
            .iter()
            .map(|(s, e)| (e - s).saturating_add(1))
            .sum()
    }

    /// Calculate how many bytes in the given [start, end] interval are already downloaded.
    pub fn bytes_in_range(&self, start: u64, end: u64) -> u64 {
        let mut sum = 0;
        for &(r_start, r_end) in &self.ranges {
            let overlap_start = start.max(r_start);
            let overlap_end = end.min(r_end);
            if overlap_end >= overlap_start {
                sum += overlap_end - overlap_start + 1;
            }
        }
        sum
    }

    /// True if the entire [0, total - 1] file has been downloaded with zero missing bytes.
    pub fn is_complete(&self) -> bool {
        if self.total == 0 {
            return true;
        }
        self.ranges.len() == 1
            && self.ranges[0].0 == 0
            && self.ranges[0].1 == self.total.saturating_sub(1)
    }

    /// Serializes range map into binary format:
    /// - 4 bytes: Magic "ODMR" (0x4F444D52)
    /// - 4 bytes: Version (u32 little-endian)
    /// - 8 bytes: Total length (u64 little-endian)
    /// - 8 bytes: ETag Hash (u64 little-endian)
    /// - 4 bytes: Range count N (u32 little-endian)
    /// - N * 16 bytes: Pairs of (start: u64, end: u64)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(28 + self.ranges.len() * 16);
        buf.extend_from_slice(&RANGE_MAP_MAGIC);
        buf.extend_from_slice(&RANGE_MAP_VERSION.to_le_bytes());
        buf.extend_from_slice(&self.total.to_le_bytes());
        buf.extend_from_slice(&self.etag_hash.to_le_bytes());
        let count = self.ranges.len() as u32;
        buf.extend_from_slice(&count.to_le_bytes());

        for &(start, end) in &self.ranges {
            buf.extend_from_slice(&start.to_le_bytes());
            buf.extend_from_slice(&end.to_le_bytes());
        }

        buf
    }

    /// Deserializes range map from binary format.
    /// Returns None if format, magic, version, total, or etag_hash does not match,
    /// or if any interval is out-of-order or extends beyond total.
    pub fn from_bytes(bytes: &[u8], expected_total: u64, expected_etag_hash: u64) -> Option<Self> {
        if bytes.len() < 28 {
            return None;
        }

        if bytes[0..4] != RANGE_MAP_MAGIC {
            return None;
        }

        let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        if version != RANGE_MAP_VERSION {
            return None;
        }

        let total = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        if total != expected_total {
            return None;
        }

        let etag_hash = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
        if expected_etag_hash != 0 && etag_hash != 0 && etag_hash != expected_etag_hash {
            return None;
        }

        let count = u32::from_le_bytes(bytes[24..28].try_into().ok()?) as usize;
        let expected_len = 28 + count * 16;
        if bytes.len() != expected_len {
            return None;
        }

        let mut ranges = Vec::with_capacity(count);
        let mut offset = 28;
        let mut prev_end: Option<u64> = None;

        for _ in 0..count {
            let start = u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?);
            let end = u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().ok()?);

            // Interval invariants
            if start > end || end >= expected_total {
                return None;
            }
            if let Some(pe) = prev_end {
                if start <= pe {
                    // Overlapping or unsorted range in stored binary
                    return None;
                }
            }
            prev_end = Some(end);
            ranges.push((start, end));
            offset += 16;
        }

        Some(Self {
            total,
            etag_hash,
            ranges,
        })
    }

    /// Atomically writes the range map to `<path>.tmp`, syncs, and renames to `<path>`.
    pub fn save_atomic(&self, path: &Path) -> io::Result<()> {
        let tmp_path = path.with_extension("ranges.tmp");
        let bytes = self.to_bytes();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;

        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);

        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Cleans up any leftover temporary files (e.g. `<path>.ranges.tmp`).
    pub fn cleanup_temp(path: &Path) {
        let tmp_path = path.with_extension("ranges.tmp");
        let _ = fs::remove_file(tmp_path);
    }

    /// Loads the range map from disk.
    /// Returns Ok(None) if file does not exist or content is corrupted / invalid.
    pub fn load(path: &Path, expected_total: u64, expected_etag_hash: u64) -> io::Result<Option<Self>> {
        Self::cleanup_temp(path);

        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };

        Ok(Self::from_bytes(&bytes, expected_total, expected_etag_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_map_merge_and_total() {
        let mut map = RangeMap::new(1000, 12345);
        map.add_range(0, 99);
        assert_eq!(map.downloaded_bytes(), 100);
        assert_eq!(map.ranges, vec![(0, 99)]);

        // Adjacent range -> merge
        map.add_range(100, 199);
        assert_eq!(map.downloaded_bytes(), 200);
        assert_eq!(map.ranges, vec![(0, 199)]);

        // Disjoint range -> separate
        map.add_range(300, 399);
        assert_eq!(map.downloaded_bytes(), 300);
        assert_eq!(map.ranges, vec![(0, 199), (300, 399)]);

        // Bridging range -> joins both
        map.add_range(150, 350);
        assert_eq!(map.downloaded_bytes(), 400);
        assert_eq!(map.ranges, vec![(0, 399)]);

        // Finish whole file
        map.add_range(400, 999);
        assert!(map.is_complete());
        assert_eq!(map.downloaded_bytes(), 1000);
        assert_eq!(map.ranges, vec![(0, 999)]);
    }

    #[test]
    fn test_range_map_serialization_round_trip() {
        let mut map = RangeMap::new(5000, 0xABCDEF0123456789);
        map.add_range(0, 499);
        map.add_range(1000, 1499);
        map.add_range(2000, 2499);

        let bytes = map.to_bytes();
        let loaded = RangeMap::from_bytes(&bytes, 5000, 0xABCDEF0123456789).expect("valid load");
        assert_eq!(map, loaded);

        // Mismatched total returns None
        assert!(RangeMap::from_bytes(&bytes, 6000, 0xABCDEF0123456789).is_none());

        // Mismatched etag hash returns None
        assert!(RangeMap::from_bytes(&bytes, 5000, 0x999).is_none());
    }

    #[test]
    fn test_range_map_rejects_corrupted_or_out_of_bounds() {
        let mut map = RangeMap::new(1000, 12345);
        map.add_range(0, 499);
        let mut bytes = map.to_bytes();

        // 1. Truncated bytes
        assert!(RangeMap::from_bytes(&bytes[..20], 1000, 12345).is_none());

        // 2. Corrupt magic
        bytes[0] = 0x00;
        assert!(RangeMap::from_bytes(&bytes, 1000, 12345).is_none());

        // 3. Out of bounds range (> total)
        let mut valid_bytes = map.to_bytes();
        // Modify end offset of range 0 to 1500 (beyond 1000 total)
        valid_bytes[36..44].copy_from_slice(&1500_u64.to_le_bytes());
        assert!(RangeMap::from_bytes(&valid_bytes, 1000, 12345).is_none());
    }

    #[test]
    fn test_range_map_atomic_file_persistence() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let path = tmp_dir.path().join("test.odm.ranges");

        let mut map = RangeMap::new(1024 * 1024, hash_etag(Some("\"etag-123\"")));
        map.add_range(0, 65535);
        map.save_atomic(&path).expect("save should succeed");

        let loaded = RangeMap::load(&path, 1024 * 1024, hash_etag(Some("\"etag-123\"")))
            .expect("load should succeed")
            .expect("map should exist");

        assert_eq!(map, loaded);
    }
}
