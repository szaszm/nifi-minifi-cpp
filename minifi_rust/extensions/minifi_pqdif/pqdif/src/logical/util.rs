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

//! Shared helpers for turning element-tree fragments into typed logical
//! record fields.

use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::error::PqdifError;
use crate::physical::element::{CollectionElement, Element};
use crate::physical::physical_type::PhysicalType;
use crate::timestamp;

/// Look up a vector-of-`Char1` child by tag and decode it as UTF-8.
///
/// PQDIF strings are `null`-terminated byte vectors. The reference
/// implementation also allows `Char2` (UTF-16); this crate transcodes both.
/// Returns `Ok(None)` when the tag is not present.
pub fn decode_string(body: &CollectionElement, tag: Uuid) -> Result<Option<String>, PqdifError> {
    let vec = match body.vector(tag) {
        Some(v) => v,
        None => return Ok(None),
    };
    match vec.ty {
        PhysicalType::Char1 => {
            let end = vec
                .values
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(vec.values.len());
            let s = std::str::from_utf8(&vec.values[..end])
                .map_err(|_| PqdifError::InvalidValue {
                    tag,
                    reason: "Char1 vector was not valid UTF-8",
                })?
                .to_owned();
            Ok(Some(s))
        }
        PhysicalType::Char2 => {
            if vec.values.len() % 2 != 0 {
                return Err(PqdifError::InvalidValue {
                    tag,
                    reason: "Char2 vector had odd byte length",
                });
            }
            let mut units: Vec<u16> = vec
                .values
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            if let Some(pos) = units.iter().position(|&u| u == 0) {
                units.truncate(pos);
            }
            let s = String::from_utf16(&units).map_err(|_| PqdifError::InvalidValue {
                tag,
                reason: "Char2 vector was not valid UTF-16",
            })?;
            Ok(Some(s))
        }
        _ => Err(PqdifError::InvalidValue {
            tag,
            reason: "string tag was not encoded as a character vector",
        }),
    }
}

/// Decode a scalar `Timestamp` child by tag. Returns `Ok(None)` when the tag
/// is not present.
pub fn timestamp_from_scalar(
    body: &CollectionElement,
    tag: Uuid,
) -> Result<Option<NaiveDateTime>, PqdifError> {
    let Some(s) = body.scalar(tag) else {
        return Ok(None);
    };
    if s.ty != PhysicalType::Timestamp {
        return Err(PqdifError::InvalidValue {
            tag,
            reason: "expected a Timestamp scalar",
        });
    }
    let decoded = timestamp::decode(&s.value).ok_or(PqdifError::InvalidValue {
        tag,
        reason: "timestamp payload was invalid",
    })?;
    Ok(Some(decoded))
}

/// Read a scalar `u32` child, or return the default when absent.
pub fn u32_or(body: &CollectionElement, tag: Uuid, default: u32) -> u32 {
    body.scalar(tag).and_then(|s| s.as_u32()).unwrap_or(default)
}

/// Read a scalar `f64` child by tag.
#[allow(dead_code)]
pub fn f64_from_scalar(body: &CollectionElement, tag: Uuid) -> Option<f64> {
    body.scalar(tag).and_then(|s| s.as_f64())
}

/// Read a scalar `Guid` child by tag.
pub fn guid_from_scalar(body: &CollectionElement, tag: Uuid) -> Option<Uuid> {
    body.scalar(tag).and_then(|s| s.as_guid())
}

/// Iterate collection children with the given tag.
pub fn collections_with_tag<'a>(
    body: &'a CollectionElement,
    tag: Uuid,
) -> impl Iterator<Item = &'a CollectionElement> + 'a {
    body.children.iter().filter_map(move |c| match c {
        Element::Collection(c) if c.tag == tag => Some(c),
        _ => None,
    })
}
