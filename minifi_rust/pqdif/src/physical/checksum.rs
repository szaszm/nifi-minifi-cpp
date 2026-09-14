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
//! not. It is stored little-endian at bytes 60..64 of the record header.

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
}
