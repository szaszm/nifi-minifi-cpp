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

//! The 64-byte record header at the start of every PQDIF record.
//!
//! Byte layout (little-endian throughout):
//!
//! | offset | length | field                                          |
//! |-------:|-------:|------------------------------------------------|
//! |     0  |    16  | `signature` — fixed GUID, same on every record |
//! |    16  |    16  | `type_tag` — record-level GUID identifying kind|
//! |    32  |     4  | `header_size` — always `64`                    |
//! |    36  |     4  | `body_size` — length of the (on-disk) body     |
//! |    40  |     4  | `next_record_pos` — absolute file offset       |
//! |    44  |    12  | reserved / unused (zero-filled)                |
//! |    56  |     4  | reserved (zero-filled)                         |
//! |    60  |     4  | `checksum` — Adler-32 over the on-disk body    |
//!
//! The signature is the same GUID every record carries so parsers can quickly
//! confirm they are reading a PQDIF file; `type_tag` is what tells the
//! logical layer whether the record is a Container, DataSource, Observation,
//! MonitorSettings, or Blank.

use std::io::Read;

use uuid::Uuid;

use crate::error::PqdifError;

/// Byte length of a record header.
pub const HEADER_LEN: usize = 64;

/// PQDIF signature GUID present at the start of every record.
///
/// The reference implementation calls this `SignatureGuid` and hard-codes it
/// as `4b1c9187-e88e-11cf-9d89-0080c72e70a3`.
pub const RECORD_SIGNATURE: Uuid = uuid::uuid!("4b1c9187-e88e-11cf-9d89-0080c72e70a3");

/// Decoded record header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordHeader {
    /// The fixed PQDIF signature. Verified against [`RECORD_SIGNATURE`].
    pub signature: Uuid,
    /// GUID identifying the record's kind (see
    /// [`crate::logical::RecordType`]).
    pub type_tag: Uuid,
    /// Length of the header itself — always [`HEADER_LEN`].
    pub header_size: u32,
    /// Length in bytes of the on-disk record body that immediately follows
    /// this header. When compression is `RecordLevel + Zlib`, this covers
    /// the compressed stream, not the decompressed size.
    pub body_size: u32,
    /// Absolute file offset of the next record. Zero means "no successor".
    pub next_record_pos: u32,
    /// Adler-32 checksum over the `body_size` on-disk body bytes.
    pub checksum: u32,
}

impl RecordHeader {
    /// Read a header from `reader`. Verifies the signature GUID and the
    /// invariant `header_size == HEADER_LEN`; returns [`PqdifError`] on any
    /// mismatch.
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self, PqdifError> {
        let mut buf = [0u8; HEADER_LEN];
        reader.read_exact(&mut buf)?;
        Self::parse(&buf)
    }

    /// Decode a header from an already-buffered 64-byte slice.
    pub fn parse(bytes: &[u8]) -> Result<Self, PqdifError> {
        if bytes.len() != HEADER_LEN {
            return Err(PqdifError::BadHeader("record header must be 64 bytes"));
        }
        let signature = read_guid(&bytes[0..16]);
        if signature != RECORD_SIGNATURE {
            return Err(PqdifError::BadSignature);
        }
        let type_tag = read_guid(&bytes[16..32]);
        let header_size = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        let body_size = u32::from_le_bytes(bytes[36..40].try_into().unwrap());
        let next_record_pos = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        // bytes[44..60] are reserved / unused. We do not validate them —
        // GSF ignores them on read and different writers have been observed
        // to leave them as garbage.
        let checksum = u32::from_le_bytes(bytes[60..64].try_into().unwrap());

        if header_size as usize != HEADER_LEN {
            return Err(PqdifError::BadHeader("header_size must be 64"));
        }

        Ok(Self {
            signature,
            type_tag,
            header_size,
            body_size,
            next_record_pos,
            checksum,
        })
    }
}

/// Read a GUID from a 16-byte little-endian buffer, matching the on-disk
/// encoding used by Windows / .NET (`Guid.ToByteArray`): the first three
/// fields are little-endian, the remaining eight bytes are big-endian.
pub(crate) fn read_guid(bytes: &[u8]) -> Uuid {
    debug_assert_eq!(bytes.len(), 16);
    let a = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let b = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    let c = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
    let d: [u8; 8] = bytes[8..16].try_into().unwrap();
    Uuid::from_fields(a, b, c, &d)
}

/// Inverse of [`read_guid`] — write a GUID into a 16-byte buffer.
///
/// Currently used only by tests and the (Phase 2) writer.
#[allow(dead_code)]
pub(crate) fn write_guid(guid: Uuid, out: &mut [u8]) {
    debug_assert_eq!(out.len(), 16);
    let (a, b, c, d) = guid.as_fields();
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..6].copy_from_slice(&b.to_le_bytes());
    out[6..8].copy_from_slice(&c.to_le_bytes());
    out[8..16].copy_from_slice(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_guid_encoding() {
        let g = uuid::uuid!("89738606-f1c3-11cf-9d89-0080c72e70a3");
        let mut buf = [0u8; 16];
        write_guid(g, &mut buf);
        assert_eq!(read_guid(&buf), g);
    }

    #[test]
    fn reject_bad_signature() {
        let mut buf = [0u8; HEADER_LEN];
        // Non-signature GUID:
        write_guid(uuid::uuid!("00000000-0000-0000-0000-000000000000"), &mut buf[0..16]);
        buf[32] = 64;
        assert!(matches!(RecordHeader::parse(&buf), Err(PqdifError::BadSignature)));
    }

    #[test]
    fn reject_wrong_header_size() {
        let mut buf = [0u8; HEADER_LEN];
        write_guid(RECORD_SIGNATURE, &mut buf[0..16]);
        buf[32] = 32; // header_size = 32, not 64
        assert!(matches!(
            RecordHeader::parse(&buf),
            Err(PqdifError::BadHeader(_))
        ));
    }

    #[test]
    fn parse_ok_header() {
        let mut buf = [0u8; HEADER_LEN];
        write_guid(RECORD_SIGNATURE, &mut buf[0..16]);
        let type_tag = uuid::uuid!("89738606-f1c3-11cf-9d89-0080c72e70a3");
        write_guid(type_tag, &mut buf[16..32]);
        buf[32..36].copy_from_slice(&64u32.to_le_bytes());
        buf[36..40].copy_from_slice(&123u32.to_le_bytes());
        buf[40..44].copy_from_slice(&456u32.to_le_bytes());
        buf[60..64].copy_from_slice(&0xdeadbeef_u32.to_le_bytes());

        let h = RecordHeader::parse(&buf).unwrap();
        assert_eq!(h.signature, RECORD_SIGNATURE);
        assert_eq!(h.type_tag, type_tag);
        assert_eq!(h.header_size, 64);
        assert_eq!(h.body_size, 123);
        assert_eq!(h.next_record_pos, 456);
        assert_eq!(h.checksum, 0xdeadbeef);
    }
}
