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

//! PQDIF timestamp decode.
//!
//! A wire-format PQDIF timestamp is 12 bytes:
//!
//! - `u32` little-endian: whole days since `1900-01-01`.
//! - `f64` little-endian: seconds within that day.
//!
//! The reference implementation (GSF.PQDIF) uses `epoch = 1900-01-01` plus
//! `days − 2`. The `− 2` corrects for a documented off-by-two in the
//! IEEE 1159.3 spec's stated `1900 → 1970 = 25569 days` constant — the real
//! gap between those two dates is `25567` days. This crate uses the same
//! canonical formula everywhere.

use chrono::{Days, NaiveDate, NaiveDateTime};

/// The `-2 days` correction spelled out in this module's docs.
const DAY_OFFSET: i64 = -2;

/// Decode a 12-byte PQDIF timestamp payload into a UTC-naive `NaiveDateTime`.
///
/// Returns `None` if the byte slice is the wrong length, if `days` cannot be
/// added to the epoch without overflow, or if `seconds` is negative / not
/// finite.
pub fn decode(bytes: &[u8]) -> Option<NaiveDateTime> {
    if bytes.len() != 12 {
        return None;
    }
    let days = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let seconds = f64::from_le_bytes(bytes[4..12].try_into().unwrap());
    from_parts(days, seconds)
}

/// Build a `NaiveDateTime` from the raw `(days, seconds)` pair. Split out so
/// tests can exercise the arithmetic without going through `decode()`.
pub fn from_parts(days: u32, seconds: f64) -> Option<NaiveDateTime> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let epoch: NaiveDate = NaiveDate::from_ymd_opt(1900, 1, 1)?;
    let corrected_days = i64::from(days).checked_add(DAY_OFFSET)?;
    let date = if corrected_days >= 0 {
        epoch.checked_add_days(Days::new(corrected_days as u64))?
    } else {
        epoch.checked_sub_days(Days::new(corrected_days.unsigned_abs()))?
    };
    let whole_seconds = seconds.trunc() as u64;
    let nanos_f = (seconds - seconds.trunc()) * 1_000_000_000.0;
    let nanos = nanos_f.round() as u32;
    let datetime = date.and_hms_opt(0, 0, 0)?;
    let with_seconds =
        datetime.checked_add_signed(chrono::TimeDelta::try_seconds(whole_seconds as i64)?)?;
    with_seconds.checked_add_signed(chrono::TimeDelta::nanoseconds(i64::from(nanos)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_plus_two_days_matches_1900_01_01() {
        // The `-2` correction means `days=2, seconds=0` maps to 1900-01-01.
        let ts = from_parts(2, 0.0).unwrap();
        assert_eq!(ts, NaiveDate::from_ymd_opt(1900, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap());
    }

    #[test]
    fn seconds_encode_time_of_day() {
        // 2 days, 12h30m45s → 1900-01-01 12:30:45.
        let ts = from_parts(2, 12.0 * 3600.0 + 30.0 * 60.0 + 45.0).unwrap();
        assert_eq!(
            ts,
            NaiveDate::from_ymd_opt(1900, 1, 1)
                .unwrap()
                .and_hms_opt(12, 30, 45)
                .unwrap()
        );
    }

    #[test]
    fn decode_rejects_wrong_length() {
        assert!(decode(&[0u8; 11]).is_none());
        assert!(decode(&[0u8; 13]).is_none());
    }

    #[test]
    fn decode_rejects_nan_and_negative() {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&2u32.to_le_bytes());
        buf[4..12].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode(&buf).is_none());

        buf[4..12].copy_from_slice(&(-1.0_f64).to_le_bytes());
        assert!(decode(&buf).is_none());
    }

    #[test]
    fn decode_round_trip() {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&44197u32.to_le_bytes()); // days
        buf[4..12].copy_from_slice(&0.0_f64.to_le_bytes()); // midnight
        // 44197 - 2 = 44195 days after 1900-01-01.
        let expected = NaiveDate::from_ymd_opt(1900, 1, 1)
            .unwrap()
            .checked_add_days(Days::new(44195))
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(decode(&buf).unwrap(), expected);
    }
}
