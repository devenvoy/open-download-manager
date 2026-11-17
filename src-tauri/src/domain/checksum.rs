// Created: 2026-11-17
// Reason: feat(checksum): whole-file checksum verification and published digest inspection
// Description: Domain representations and parsing logic for file checksums and server digests.

use base64::Engine as _;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumAlgorithm {
    Crc32,
    Sha256,
    Sha512,
}

impl ChecksumAlgorithm {
    pub fn name(self) -> &'static str {
        match self {
            ChecksumAlgorithm::Crc32 => "CRC32",
            ChecksumAlgorithm::Sha256 => "SHA-256",
            ChecksumAlgorithm::Sha512 => "SHA-512",
        }
    }

    pub fn digest_len(self) -> usize {
        match self {
            ChecksumAlgorithm::Crc32 => 4,
            ChecksumAlgorithm::Sha256 => 32,
            ChecksumAlgorithm::Sha512 => 64,
        }
    }

    pub fn hex_len(self) -> usize {
        self.digest_len() * 2
    }

    pub fn strength(self) -> u8 {
        match self {
            ChecksumAlgorithm::Crc32 => 1,
            ChecksumAlgorithm::Sha256 => 2,
            ChecksumAlgorithm::Sha512 => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checksum {
    pub algorithm: ChecksumAlgorithm,
    #[serde(with = "hex_bytes")]
    pub value: Vec<u8>,
}

impl Checksum {
    pub fn new(algorithm: ChecksumAlgorithm, value: Vec<u8>) -> Option<Self> {
        if value.len() == algorithm.digest_len() {
            Some(Self { algorithm, value })
        } else {
            None
        }
    }

    /// Parses a user-supplied checksum string.
    /// Supports explicit prefix like `sha256:<hex>` or `sha-256=<hex>` or `crc32:<hex>`.
    /// In the absence of a prefix, the algorithm is deduced from the hex string length:
    /// - 8 characters (4 bytes): CRC32
    /// - 64 characters (32 bytes): SHA-256
    /// - 128 characters (64 bytes): SHA-512
    /// Ignores whitespace, casing, and trailing filenames (e.g. `sha256sum` output format).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        let (prefix_algo, rest) = match text.split_once([':', '=']) {
            Some((prefix, remainder)) if prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => {
                let norm = prefix.to_ascii_lowercase().replace('-', "");
                let algo = match norm.as_str() {
                    "crc32" => Some(ChecksumAlgorithm::Crc32),
                    "sha256" => Some(ChecksumAlgorithm::Sha256),
                    "sha512" => Some(ChecksumAlgorithm::Sha512),
                    _ => None,
                };
                if algo.is_some() {
                    (algo, remainder)
                } else {
                    (None, text)
                }
            }
            _ => (None, text),
        };

        // Extract hex characters from the first non-whitespace segment (handling standard sha256sum tool output)
        let token = rest.split_whitespace().next()?;
        let hex_str: String = token.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if hex_str.len() % 2 != 0 {
            return None;
        }

        let raw_bytes = hex::decode(&hex_str).ok()?;
        let algorithm = match prefix_algo {
            Some(a) => a,
            None => match raw_bytes.len() {
                4 => ChecksumAlgorithm::Crc32,
                32 => ChecksumAlgorithm::Sha256,
                64 => ChecksumAlgorithm::Sha512,
                _ => return None,
            },
        };

        if raw_bytes.len() == algorithm.digest_len() {
            Some(Self {
                algorithm,
                value: raw_bytes,
            })
        } else {
            None
        }
    }

    /// Parses published server digest headers (RFC 9530 `Repr-Digest`, RFC 3230 `Digest`).
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        let b64 = |s: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(s.trim().trim_matches(':'))
                .ok()
        };

        let mut candidates = Vec::new();

        // 1. Check Repr-Digest and Digest headers
        for name in ["repr-digest", "digest"] {
            for v in headers.get_all(name).iter().filter_map(|v| v.to_str().ok()) {
                for part in v.split(',') {
                    let Some((algo_name, val)) = part.trim().split_once('=') else {
                        continue;
                    };
                    let algo = match algo_name.trim().to_ascii_lowercase().as_str() {
                        "sha-512" => ChecksumAlgorithm::Sha512,
                        "sha-256" => ChecksumAlgorithm::Sha256,
                        _ => continue,
                    };
                    if let Some(bytes) = b64(val).filter(|b| b.len() == algo.digest_len()) {
                        candidates.push(Checksum {
                            algorithm: algo,
                            value: bytes,
                        });
                    }
                }
            }
        }

        // Return the strongest announced algorithm
        candidates.into_iter().max_by_key(|c| c.algorithm.strength())
    }

    pub fn hex(&self) -> String {
        hex::encode(&self.value)
    }

    pub fn display(&self) -> String {
        format!("{}:{}", self.algorithm.name().to_ascii_lowercase().replace('-', ""), self.hex())
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_checksum_by_length() {
        // CRC32 (8 hex chars = 4 bytes)
        let crc = Checksum::parse("1a2b3c4d").unwrap();
        assert_eq!(crc.algorithm, ChecksumAlgorithm::Crc32);
        assert_eq!(crc.hex(), "1a2b3c4d");

        // SHA-256 (64 hex chars = 32 bytes)
        let sha256_hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let sha256 = Checksum::parse(sha256_hex).unwrap();
        assert_eq!(sha256.algorithm, ChecksumAlgorithm::Sha256);
        assert_eq!(sha256.hex(), sha256_hex);

        // Invalid length
        assert!(Checksum::parse("12345").is_none());
    }

    #[test]
    fn test_parse_checksum_with_prefix() {
        let s = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let c = Checksum::parse(s).unwrap();
        assert_eq!(c.algorithm, ChecksumAlgorithm::Sha256);

        let s2 = "SHA-256=e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let c2 = Checksum::parse(s2).unwrap();
        assert_eq!(c2.algorithm, ChecksumAlgorithm::Sha256);

        let s3 = "crc32:1A2B3C4D";
        let c3 = Checksum::parse(s3).unwrap();
        assert_eq!(c3.algorithm, ChecksumAlgorithm::Crc32);
        assert_eq!(c3.hex(), "1a2b3c4d");
    }

    #[test]
    fn test_parse_checksum_with_sha256sum_filename() {
        let line = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  *ubuntu-24.04.iso";
        let c = Checksum::parse(line).unwrap();
        assert_eq!(c.algorithm, ChecksumAlgorithm::Sha256);
        assert_eq!(c.hex(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_from_headers() {
        let mut headers = HeaderMap::new();
        // Base64 for 32 zeros: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        headers.insert(
            "repr-digest",
            "sha-256=:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=:".parse().unwrap(),
        );
        let c = Checksum::from_headers(&headers).unwrap();
        assert_eq!(c.algorithm, ChecksumAlgorithm::Sha256);
        assert_eq!(c.value, vec![0u8; 32]);
    }
}
