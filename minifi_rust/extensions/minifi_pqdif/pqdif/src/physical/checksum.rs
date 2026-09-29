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

//! Adler-32 checksum used by PQDIF record headers.
//!
//! PQDIF's `RecordBody.cs` in `GSF.PQDIF` misleadingly documents the checksum
//! as "CRC32" — the code and the interoperable field on disk are both
//! Adler-32 (`GSF.IO.Checksums.Adler32` in gsf; `Ionic.Zlib.Adler.Adler32` in
//! `gemstone/pqdif`). This module implements the plain RFC 1950 Adler-32.
//!
//! The checksum is computed over the **on-disk** body bytes: the compressed
//! byte stream when compression is active, and the raw body bytes when it is
//! not. It is stored little-endian at bytes 44..48 of the record header.

/// Compute the Adler-32 checksum of `data` per RFC 1950 § 9.
pub fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

/// When [`crate::physical::PhysicalParser`] should verify a record's stored
/// Adler-32 checksum against the body bytes it just read.
///
/// PQDIF has no "checksum absent" flag, so writers that do not compute one
/// simply leave the field `0x00000000`. That makes the choice a policy
/// question rather than a format question, hence this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChecksumMode {
    /// Verify every record. A writer that left the field zero is treated as
    /// a corrupt file.
    ///
    /// This is the default, and is deliberately stricter than the reference
    /// implementation: `GSF.PQDIF` computes the checksum on read but never
    /// compares it. Appropriate for input from untrusted sources.
    #[default]
    Always,
    /// Verify only records whose stored checksum is non-zero; accept a zero
    /// field as "this writer did not compute one".
    ///
    /// Intended for real-world files from *trusted* sources, where writers
    /// vary in whether they populate the field. Corruption is still caught
    /// in every record that carries a checksum.
    ///
    /// Note the two limits of treating zero as a sentinel. A corrupt body
    /// whose header also reads zero is indistinguishable from an
    /// uncomputed checksum and passes. And in the other direction, Adler-32
    /// can in principle produce `0x00000000` for a contrived body, whose
    /// (genuine, matching) checksum this mode would then skip — harmless,
    /// and unreachable for realistic record bodies.
    WhenPresent,
    /// Never verify. Read the field into [`super::RecordHeader::checksum`]
    /// and leave any policy to the caller.
    Never,
}

impl ChecksumMode {
    /// Whether a record whose header carries `stored_checksum` should have
    /// its body verified under this mode.
    pub fn should_verify(self, stored_checksum: u32) -> bool {
        match self {
            Self::Always => true,
            Self::WhenPresent => stored_checksum != 0,
            Self::Never => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_matches_reference() {
        // Adler-32 of the empty string is 1.
        assert_eq!(adler32(&[]), 1);
    }

    #[test]
    fn wikipedia_reference_vector() {
        // "Wikipedia" → 0x11E60398.
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn tampering_changes_output() {
        let clean = adler32(b"abcdefgh");
        let tampered = adler32(b"abcdefgi");
        assert_ne!(clean, tampered);
    }

    #[test]
    fn always_verifies_regardless_of_stored_value() {
        assert!(ChecksumMode::Always.should_verify(0));
        assert!(ChecksumMode::Always.should_verify(0x2acc_10e4));
    }

    #[test]
    fn when_present_treats_zero_as_absent() {
        assert!(!ChecksumMode::WhenPresent.should_verify(0));
        assert!(ChecksumMode::WhenPresent.should_verify(1));
        assert!(ChecksumMode::WhenPresent.should_verify(0x2acc_10e4));
    }

    #[test]
    fn never_skips_everything() {
        assert!(!ChecksumMode::Never.should_verify(0));
        assert!(!ChecksumMode::Never.should_verify(0x2acc_10e4));
    }

    #[test]
    fn default_is_the_strict_mode() {
        assert_eq!(ChecksumMode::default(), ChecksumMode::Always);
    }
}
