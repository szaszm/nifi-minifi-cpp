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

//! Error type for the `pqdif` crate.

use thiserror::Error;
use uuid::Uuid;

/// Every failure mode surfaced by the physical or logical parser.
///
/// The variants distinguish between transport failures (`Io`), format-level
/// problems (`BadSignature`, `BadHeader`, `Truncated`, `UnknownPhysicalType`,
/// `UnknownElementType`), integrity failures (`ChecksumMismatch`),
/// intentionally-unsupported features (`UnsupportedCompression`),
/// robustness guards (`RecordCycle`), and logical-layer schema violations
/// (`MissingRequiredTag`, `InvalidValue`).
#[derive(Debug, Error)]
pub enum PqdifError {
    /// The underlying reader failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The record signature did not match the expected `signature` value.
    #[error("not a PQDIF file: record signature mismatch")]
    BadSignature,

    /// The record header failed a structural sanity check.
    #[error("invalid record header: {0}")]
    BadHeader(&'static str),

    /// The physical-type byte in an element header was not one of the 17 valid
    /// PQDIF physical types.
    #[error("unsupported physical type byte: {0}")]
    UnknownPhysicalType(u8),

    /// The element-type byte in an element header was not `Scalar` (1),
    /// `Vector` (2), or `Collection` (3).
    #[error("unsupported element type byte: {0}")]
    UnknownElementType(u8),

    /// The container record requested a compression mode this crate does not
    /// implement: `TotalFile` (deprecated by PQDIF 1.5), or `PKZIP`.
    #[error("unsupported compression: {0}")]
    UnsupportedCompression(&'static str),

    /// A `next_record_pos` link pointed to a record offset already visited on
    /// this walk. Prevents malformed / adversarial files from causing an
    /// infinite loop.
    #[error("record chain cycle detected at offset {0}")]
    RecordCycle(u64),

    /// The Adler-32 checksum stored in a record header did not match the
    /// checksum computed over the on-disk body bytes.
    #[error("checksum mismatch (expected {expected:#x}, got {actual:#x})")]
    ChecksumMismatch {
        /// Value stored in the record header.
        expected: u32,
        /// Value computed over the body bytes.
        actual: u32,
    },

    /// The record body ended before all of its elements had been decoded, or
    /// a link offset pointed past the end of the body.
    #[error("truncated record body")]
    Truncated,

    /// The logical layer expected a required tag inside a record but did not
    /// find it (for example the `WriterName` scalar inside a Container).
    #[error("required tag {0} missing")]
    MissingRequiredTag(Uuid),

    /// A tag value fell outside the range the logical layer accepts (for
    /// example a `SeriesInstance` whose `SeriesShareChannelIndex` refers to a
    /// nonexistent series).
    #[error("invalid value for tag {tag}: {reason}")]
    InvalidValue {
        /// The tag whose value was rejected.
        tag: Uuid,
        /// Human-readable reason.
        reason: &'static str,
    },
}
