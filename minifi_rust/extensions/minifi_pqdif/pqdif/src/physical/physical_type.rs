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

//! The 17 PQDIF physical types (§ Storage-Level Definitions of IEEE 1159.3).

use crate::error::PqdifError;

/// One of the seventeen physical types PQDIF supports for storing values on
/// the wire. The numeric discriminants match the wire encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PhysicalType {
    /// Single-byte boolean.
    Boolean1 = 1,
    /// 16-bit boolean.
    Boolean2 = 2,
    /// 32-bit boolean.
    Boolean4 = 3,
    /// Single-byte narrow character.
    Char1 = 10,
    /// UTF-16 wide character.
    Char2 = 11,
    /// Signed 8-bit integer.
    Integer1 = 20,
    /// Signed 16-bit integer.
    Integer2 = 21,
    /// Signed 32-bit integer.
    Integer4 = 22,
    /// Unsigned 8-bit integer.
    UnsignedInteger1 = 30,
    /// Unsigned 16-bit integer.
    UnsignedInteger2 = 31,
    /// Unsigned 32-bit integer.
    UnsignedInteger4 = 32,
    /// 32-bit IEEE 754 single-precision.
    Real4 = 40,
    /// 64-bit IEEE 754 double-precision.
    Real8 = 41,
    /// A pair of `Real4` values (`re`, `im`).
    Complex8 = 42,
    /// A pair of `Real8` values (`re`, `im`).
    Complex16 = 43,
    /// 12-byte PQDIF timestamp — see [`crate::timestamp`].
    Timestamp = 50,
    /// 16-byte GUID.
    Guid = 60,
}

impl PhysicalType {
    /// Decode a physical-type byte read from an element header.
    pub fn from_byte(b: u8) -> Result<Self, PqdifError> {
        Ok(match b {
            1 => Self::Boolean1,
            2 => Self::Boolean2,
            3 => Self::Boolean4,
            10 => Self::Char1,
            11 => Self::Char2,
            20 => Self::Integer1,
            21 => Self::Integer2,
            22 => Self::Integer4,
            30 => Self::UnsignedInteger1,
            31 => Self::UnsignedInteger2,
            32 => Self::UnsignedInteger4,
            40 => Self::Real4,
            41 => Self::Real8,
            42 => Self::Complex8,
            43 => Self::Complex16,
            50 => Self::Timestamp,
            60 => Self::Guid,
            other => return Err(PqdifError::UnknownPhysicalType(other)),
        })
    }

    /// Size in bytes of one value of this physical type on the wire.
    pub const fn byte_size(self) -> usize {
        match self {
            Self::Boolean1 | Self::Char1 | Self::Integer1 | Self::UnsignedInteger1 => 1,
            Self::Boolean2 | Self::Char2 | Self::Integer2 | Self::UnsignedInteger2 => 2,
            Self::Boolean4 | Self::Integer4 | Self::UnsignedInteger4 | Self::Real4 => 4,
            Self::Real8 | Self::Complex8 => 8,
            Self::Complex16 => 16,
            Self::Timestamp => 12,
            Self::Guid => 16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_reference_spec() {
        assert_eq!(PhysicalType::Boolean1.byte_size(), 1);
        assert_eq!(PhysicalType::Boolean4.byte_size(), 4);
        assert_eq!(PhysicalType::Real4.byte_size(), 4);
        assert_eq!(PhysicalType::Real8.byte_size(), 8);
        assert_eq!(PhysicalType::Complex8.byte_size(), 8);
        assert_eq!(PhysicalType::Complex16.byte_size(), 16);
        assert_eq!(PhysicalType::Timestamp.byte_size(), 12);
        assert_eq!(PhysicalType::Guid.byte_size(), 16);
    }

    #[test]
    fn from_byte_round_trips_all_known_types() {
        let all = [
            PhysicalType::Boolean1,
            PhysicalType::Boolean2,
            PhysicalType::Boolean4,
            PhysicalType::Char1,
            PhysicalType::Char2,
            PhysicalType::Integer1,
            PhysicalType::Integer2,
            PhysicalType::Integer4,
            PhysicalType::UnsignedInteger1,
            PhysicalType::UnsignedInteger2,
            PhysicalType::UnsignedInteger4,
            PhysicalType::Real4,
            PhysicalType::Real8,
            PhysicalType::Complex8,
            PhysicalType::Complex16,
            PhysicalType::Timestamp,
            PhysicalType::Guid,
        ];
        for t in all {
            let b = t as u8;
            assert_eq!(PhysicalType::from_byte(b).unwrap(), t);
        }
    }

    #[test]
    fn from_byte_rejects_unknown() {
        assert!(matches!(
            PhysicalType::from_byte(0),
            Err(PqdifError::UnknownPhysicalType(0))
        ));
        assert!(matches!(
            PhysicalType::from_byte(255),
            Err(PqdifError::UnknownPhysicalType(255))
        ));
    }
}
