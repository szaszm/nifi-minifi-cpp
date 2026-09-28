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

//! Physical-layer parser: walks the linked list of records and returns each
//! one as a decoded element tree.

use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};

use crate::error::PqdifError;
use crate::physical::checksum::{ChecksumMode, adler32};
use crate::physical::compression::{
    CompressionAlgorithm, CompressionStyle, decode_body as decompress,
};
use crate::physical::element::{CollectionElement, decode_body};
use crate::physical::header::RecordHeader;

/// One decoded record as produced by [`PhysicalParser`].
///
/// Holds both the header (so callers can see `type_tag`, sizes, checksum,
/// etc.) and the fully-decoded body element tree.
#[derive(Debug, Clone)]
pub struct PhysicalRecord {
    /// The 64-byte record header.
    pub header: RecordHeader,
    /// Absolute file offset of the record's first byte (`signature`).
    pub offset: u64,
    /// The record body decoded as a virtual top-level Collection whose
    /// `children` are the record's top-level tags. The virtual collection's
    /// own `tag` is [`uuid::Uuid::nil()`] — the meaningful tag is
    /// `header.type_tag`.
    pub body: CollectionElement,
}

/// Iterator-shaped parser over the physical layer of a PQDIF file.
///
/// Construct with [`PhysicalParser::new`], then repeatedly call
/// [`PhysicalParser::next_record`] until it returns `Ok(None)`. Records are
/// yielded in file order — starting with the Container record at offset 0
/// and following each header's `next_record_pos` pointer.
///
/// Cycle detection is unconditional: revisiting an offset already yielded
/// this walk produces [`PqdifError::RecordCycle`]. Checksum verification
/// defaults to [`ChecksumMode::Always`]; change it with
/// [`PhysicalParser::checksum_mode`].
pub struct PhysicalParser<R: Read + Seek> {
    reader: R,
    next_offset: Option<u64>,
    visited: HashSet<u64>,
    /// Compression settings resolved from the Container record. `None` until
    /// the Container has been parsed (i.e. before the first `next_record()`
    /// call). The Container record body itself is always uncompressed.
    compression: Option<(CompressionStyle, CompressionAlgorithm)>,
    checksum_mode: ChecksumMode,
    records_yielded: u64,
}

impl<R: Read + Seek> PhysicalParser<R> {
    /// Create a new parser positioned at the start of the file.
    pub fn new(mut reader: R) -> Result<Self, PqdifError> {
        reader.seek(SeekFrom::Start(0))?;
        Ok(Self {
            reader,
            next_offset: Some(0),
            visited: HashSet::new(),
            compression: None,
            checksum_mode: ChecksumMode::default(),
            records_yielded: 0,
        })
    }

    /// Set the Adler-32 checksum verification policy.
    ///
    /// Defaults to [`ChecksumMode::Always`], which is deliberately stricter
    /// than the reference implementation (it computes the checksum on read
    /// but never compares it). Use [`ChecksumMode::WhenPresent`] for
    /// real-world files from trusted sources, where writers vary in whether
    /// they populate the field.
    pub fn checksum_mode(&mut self, mode: ChecksumMode) -> &mut Self {
        self.checksum_mode = mode;
        self
    }

    /// Enable or disable checksum verification.
    ///
    /// Shorthand for [`ChecksumMode::Always`] / [`ChecksumMode::Never`]; use
    /// [`Self::checksum_mode`] to reach [`ChecksumMode::WhenPresent`].
    pub fn verify_checksums(&mut self, verify: bool) -> &mut Self {
        self.checksum_mode(if verify {
            ChecksumMode::Always
        } else {
            ChecksumMode::Never
        })
    }

    /// Read and decode the next record. Returns `Ok(None)` when the chain
    /// terminates (either explicit `next_record_pos == 0` or the reader
    /// hits EOF at the natural end of the file).
    pub fn next_record(&mut self) -> Result<Option<PhysicalRecord>, PqdifError> {
        let Some(offset) = self.next_offset else {
            return Ok(None);
        };
        if !self.visited.insert(offset) {
            return Err(PqdifError::RecordCycle(offset));
        }

        self.reader.seek(SeekFrom::Start(offset))?;
        let header = RecordHeader::read_from(&mut self.reader)?;

        // Read the on-disk body bytes.
        let mut on_disk_body = vec![0u8; header.body_size as usize];
        self.reader.read_exact(&mut on_disk_body)?;

        if self.checksum_mode.should_verify(header.checksum) {
            let actual = adler32(&on_disk_body);
            if actual != header.checksum {
                return Err(PqdifError::ChecksumMismatch {
                    expected: header.checksum,
                    actual,
                });
            }
        }

        // Container record (records_yielded == 0) is always uncompressed —
        // its body carries the compression settings for everything after it.
        let decoded_body = if self.records_yielded == 0 {
            on_disk_body
        } else {
            let (style, algo) = self.compression.unwrap_or((
                CompressionStyle::None,
                CompressionAlgorithm::None,
            ));
            decompress(style, algo, &on_disk_body)?
        };

        let body = decode_body(&decoded_body)?;

        if self.records_yielded == 0 {
            self.compression = Some(resolve_compression(&body)?);
        }

        self.next_offset = if header.next_record_pos == 0 {
            None
        } else {
            Some(u64::from(header.next_record_pos))
        };
        self.records_yielded = self.records_yielded.saturating_add(1);

        Ok(Some(PhysicalRecord {
            header,
            offset,
            body,
        }))
    }
}

fn resolve_compression(
    container_body: &CollectionElement,
) -> Result<(CompressionStyle, CompressionAlgorithm), PqdifError> {
    use crate::tag::{TAG_COMPRESSION_ALGORITHM_TAG, TAG_COMPRESSION_STYLE_TAG};

    // Compression style — optional, defaults to None.
    let style = if let Some(s) = container_body.scalar(TAG_COMPRESSION_STYLE_TAG) {
        let v = s.as_u32().ok_or(PqdifError::BadHeader(
            "CompressionStyleTag must be UnsignedInteger4",
        ))?;
        CompressionStyle::from_u32(v)?
    } else {
        CompressionStyle::None
    };

    let algorithm = if let Some(s) = container_body.scalar(TAG_COMPRESSION_ALGORITHM_TAG) {
        let v = s.as_u32().ok_or(PqdifError::BadHeader(
            "CompressionAlgorithmTag must be UnsignedInteger4",
        ))?;
        CompressionAlgorithm::from_u32(v)?
    } else {
        CompressionAlgorithm::None
    };

    Ok((style, algorithm))
}
