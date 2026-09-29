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

//! Decode the `SeriesValues` element into a `Vec<f64>` with scale/offset
//! applied.
//!
//! Two encodings are supported:
//!
//! - **Explicit** — the values are stored as a vector of the series's
//!   physical type. `Vec` length = number of samples.
//! - **Compact start/count/increment** — a 3-element vector whose entries
//!   (`start`, `count`, `increment`) generate an arithmetic progression.
//!   Selected explicitly via the `compact` parameter — the caller looks up
//!   `tagStorageMethodID` on the series *definition* to decide.
//!
//! Both encodings feed through the same `value * scale + offset` transform
//! before being returned.

use crate::error::PqdifError;
use crate::physical::element::{Element, VectorElement};
use crate::physical::physical_type::PhysicalType;

/// Decoded numeric values for one series, in file order, with scale + offset
/// applied.
pub type SeriesValues = Vec<f64>;

/// Decode the `SeriesValues` element out of a series-instance body.
///
/// * `element` — the raw `tagSeriesValues` element on the series instance.
/// * `scale`, `offset` — the `tagSeriesScale` / `tagSeriesOffset` scalars
///   from the same series-instance (both default to `1.0` and `0.0` when
///   absent).
/// * `compact` — `true` iff the series definition's `tagStorageMethodID`
///   indicates the compact `(start, count, increment)` encoding.
pub fn decode(
    element: &Element,
    scale: f64,
    offset: f64,
    compact: bool,
) -> Result<SeriesValues, PqdifError> {
    let vec = match element {
        Element::Vector(v) => v,
        _ => {
            return Err(PqdifError::BadHeader(
                "SeriesValues element must be a Vector",
            ));
        }
    };

    let values = read_typed_vector(vec)?;
    let raw = if compact {
        if values.len() != 3 {
            return Err(PqdifError::BadHeader(
                "compact SeriesValues must have length 3",
            ));
        }
        let start = values[0];
        let count = values[1];
        let increment = values[2];
        if !count.is_finite() || count < 0.0 || count > u32::MAX as f64 {
            return Err(PqdifError::BadHeader(
                "compact SeriesValues had invalid `count`",
            ));
        }
        let n = count as usize;
        (0..n).map(|i| start + increment * i as f64).collect()
    } else {
        values
    };

    Ok(raw.into_iter().map(|v| v * scale + offset).collect())
}

/// Decode the whole vector as `f64`s, regardless of its stored physical
/// type.
fn read_typed_vector(vec: &VectorElement) -> Result<Vec<f64>, PqdifError> {
    let ty = vec.ty;
    let bytes = &vec.values;
    let sz = ty.byte_size();
    let count = bytes.len().checked_div(sz).unwrap_or(0);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let chunk = &bytes[i * sz..(i + 1) * sz];
        let v: f64 = match ty {
            PhysicalType::Integer1 => i8::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::Integer2 => i16::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::Integer4 => i32::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::UnsignedInteger1 => u8::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::UnsignedInteger2 => u16::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::UnsignedInteger4 => u32::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::Real4 => f32::from_le_bytes(chunk.try_into().unwrap()) as f64,
            PhysicalType::Real8 => f64::from_le_bytes(chunk.try_into().unwrap()),
            _ => {
                return Err(PqdifError::BadHeader(
                    "SeriesValues vector had non-numeric physical type",
                ));
            }
        };
        out.push(v);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn f64_vec(values: &[f64]) -> Element {
        let mut bytes = Vec::with_capacity(values.len() * 8);
        for v in values {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        Element::Vector(VectorElement {
            tag: Uuid::nil(),
            ty: PhysicalType::Real8,
            values: bytes,
        })
    }

    #[test]
    fn explicit_decode_applies_scale_and_offset() {
        let el = f64_vec(&[1.0, 2.0, 3.0]);
        let got = decode(&el, 10.0, 100.0, false).unwrap();
        assert_eq!(got, vec![110.0, 120.0, 130.0]);
    }

    #[test]
    fn compact_expands_to_progression() {
        // start=5, count=4, increment=2 → [5, 7, 9, 11], then scale 1, offset 0.
        let el = f64_vec(&[5.0, 4.0, 2.0]);
        let got = decode(&el, 1.0, 0.0, true).unwrap();
        assert_eq!(got, vec![5.0, 7.0, 9.0, 11.0]);
    }

    #[test]
    fn compact_length_must_be_three() {
        let el = f64_vec(&[1.0, 2.0]);
        assert!(matches!(
            decode(&el, 1.0, 0.0, true),
            Err(PqdifError::BadHeader(_))
        ));
    }
}
