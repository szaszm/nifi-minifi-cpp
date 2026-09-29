// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Per-record compression handling.
//!
//! PQDIF supports three "compression styles" and three "compression
//! algorithms". This crate implements the modern combination —
//! [`CompressionStyle::RecordLevel`] + [`CompressionAlgorithm::Zlib`] — and
//! the trivial [`CompressionStyle::None`] one. The two deprecated modes
//! (`TotalFile` and `PKZIP`) are rejected at file open with
//! [`PqdifError::UnsupportedCompression`], mirroring GSF's
//! `NotSupportedException`.
//!
//! The zlib format used here is **RFC 1950** (2-byte header + deflate +
//! 4-byte Adler-32 trailer), i.e. `flate2::read::ZlibDecoder` — not the raw
//! deflate stream (`DeflateDecoder`) and not the gzip wrapper (`GzDecoder`).

use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::error::PqdifError;
use crate::tag::{compression_algorithm, compression_style};

/// Where compression is applied inside the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStyle {
    /// No compression — record bodies are stored verbatim.
    None,
    /// Each record body is compressed independently. Checksums in the record
    /// header cover the compressed bytes.
    RecordLevel,
}

impl CompressionStyle {
    /// Decode the `<tagCompressionStyleID>` enumeration value.
    pub fn from_u32(v: u32) -> Result<Self, PqdifError> {
        match v {
            compression_style::NONE => Ok(Self::None),
            compression_style::RECORD_LEVEL => Ok(Self::RecordLevel),
            compression_style::TOTAL_FILE => Err(PqdifError::UnsupportedCompression(
                "TotalFile compression style (deprecated in PQDIF 1.5)",
            )),
            _ => Err(PqdifError::UnsupportedCompression(
                "unknown compression style",
            )),
        }
    }
}

/// The compression algorithm applied when a style other than `None` is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// No compression algorithm — the algorithm ID is `None`.
    None,
    /// RFC 1950 zlib (deflate wrapped with an Adler-32 trailer).
    Zlib,
}

impl CompressionAlgorithm {
    /// Decode the `<tagCompressionAlgorithmID>` enumeration value.
    pub fn from_u32(v: u32) -> Result<Self, PqdifError> {
        match v {
            compression_algorithm::NONE => Ok(Self::None),
            compression_algorithm::ZLIB => Ok(Self::Zlib),
            compression_algorithm::PKZIP => Err(PqdifError::UnsupportedCompression(
                "PKZIP compression algorithm (deprecated in PQDIF 1.5)",
            )),
            _ => Err(PqdifError::UnsupportedCompression(
                "unknown compression algorithm",
            )),
        }
    }
}

/// Decompress a record body according to the container's compression
/// settings. `raw` is the on-disk body-byte slice — either the compressed
/// stream or, for `None`, the uncompressed body.
///
/// The container record is always uncompressed regardless of settings; the
/// parser therefore only calls this on records *after* the container.
pub fn decode_body(
    style: CompressionStyle,
    algorithm: CompressionAlgorithm,
    raw: &[u8],
) -> Result<Vec<u8>, PqdifError> {
    match (style, algorithm) {
        (CompressionStyle::None, _) => Ok(raw.to_vec()),
        (CompressionStyle::RecordLevel, CompressionAlgorithm::None) => Ok(raw.to_vec()),
        (CompressionStyle::RecordLevel, CompressionAlgorithm::Zlib) => {
            let mut decoded = Vec::with_capacity(raw.len());
            ZlibDecoder::new(raw).read_to_end(&mut decoded)?;
            Ok(decoded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_zlib() {
        let payload = b"the quick brown fox jumps over the lazy dog".repeat(10);
        // Compress with flate2's encoder so we do not fake wire bytes.
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload).unwrap();
        let compressed = encoder.finish().unwrap();

        let decoded = decode_body(
            CompressionStyle::RecordLevel,
            CompressionAlgorithm::Zlib,
            &compressed,
        )
        .unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn none_is_passthrough() {
        let payload = b"hello".to_vec();
        let decoded =
            decode_body(CompressionStyle::None, CompressionAlgorithm::Zlib, &payload).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn total_file_rejected() {
        assert!(matches!(
            CompressionStyle::from_u32(compression_style::TOTAL_FILE),
            Err(PqdifError::UnsupportedCompression(_))
        ));
    }

    #[test]
    fn pkzip_rejected() {
        assert!(matches!(
            CompressionAlgorithm::from_u32(compression_algorithm::PKZIP),
            Err(PqdifError::UnsupportedCompression(_))
        ));
    }
}
