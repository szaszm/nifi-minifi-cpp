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

//! Elements inside a record body.
//!
//! Every record body is a Collection element whose children are the record's
//! top-level tags. Each element on the wire has a 28-byte header:
//!
//! | offset | length | field                                                    |
//! |-------:|-------:|----------------------------------------------------------|
//! |     0  |    16  | `tag` GUID                                                |
//! |    16  |     1  | element type — 1=Collection, 2=Scalar, 3=Vector           |
//! |    17  |     1  | physical type — see [`crate::physical::PhysicalType`]     |
//! |    18  |     1  | `isEmbedded` — 1 iff a scalar's value fits in the payload |
//! |    19  |     1  | reserved / unused                                         |
//! |    20  |     8  | payload slot — embedded value **or** `(link_offset, padded_size)` |
//!
//! # Embedding rule (strictly `< 8`)
//!
//! A scalar element is embedded — i.e. its bytes live directly in the payload
//! slot at offsets 20..28 — iff its physical type's byte size is
//! **strictly less than 8**. `Real8`, `Complex8`, `Complex16`, `Timestamp`,
//! and `Guid` (all ≥ 8 B) are never embedded even when scalar. Vectors and
//! Collections are never embedded regardless of size.
//!
//! # Link semantics (non-embedded elements)
//!
//! For non-embedded elements the 8-byte payload slot is
//! `(link_offset: i32, padded_size: i32)`, both little-endian:
//!
//! - `link_offset` is a byte offset relative to the start of the **decoded**
//!   record body (not the file, not the element header).
//! - `padded_size` is the raw payload size rounded up to a multiple of 4:
//!   `padded_size = ((raw_size + 3) / 4) * 4`.
//!
//! At `link_offset`:
//! - Scalars ≥ 8 B: the value bytes, followed by up to 3 pad bytes.
//! - Vectors: `Int32` count, then `count × byte_size` contiguous raw values.
//! - Collections: `Int32` count, then `count` × 28-byte child element headers
//!   whose links refer back into this same body.

use uuid::Uuid;

use crate::error::PqdifError;
use crate::physical::header::read_guid;
use crate::physical::physical_type::PhysicalType;

/// Byte length of a single element header.
pub const ELEMENT_HEADER_LEN: usize = 28;

// Element-type discriminants match the reference `GSF.PQDIF/Physical/Element.cs`
// enum: `Collection = 1, Scalar = 2, Vector = 3`.
const ELEMENT_TYPE_COLLECTION: u8 = 1;
const ELEMENT_TYPE_SCALAR: u8 = 2;
const ELEMENT_TYPE_VECTOR: u8 = 3;

/// One node in the element tree.
#[derive(Debug, Clone)]
pub enum Element {
    /// A single value.
    Scalar(ScalarElement),
    /// An ordered list of values, all of the same physical type.
    Vector(VectorElement),
    /// A nested element tree.
    Collection(CollectionElement),
}

impl Element {
    /// GUID identifying this element's role.
    pub fn tag(&self) -> Uuid {
        match self {
            Self::Scalar(s) => s.tag,
            Self::Vector(v) => v.tag,
            Self::Collection(c) => c.tag,
        }
    }
}

/// A single-value element.
#[derive(Debug, Clone)]
pub struct ScalarElement {
    /// Tag GUID.
    pub tag: Uuid,
    /// Physical type of the value.
    pub ty: PhysicalType,
    /// The raw value bytes — exactly `ty.byte_size()` of them.
    pub value: Vec<u8>,
}

impl ScalarElement {
    /// Interpret the value as a signed 32-bit integer.
    pub fn as_i32(&self) -> Option<i32> {
        if self.ty != PhysicalType::Integer4 || self.value.len() != 4 {
            return None;
        }
        Some(i32::from_le_bytes(self.value[..4].try_into().ok()?))
    }

    /// Interpret the value as an unsigned 32-bit integer.
    pub fn as_u32(&self) -> Option<u32> {
        if self.ty != PhysicalType::UnsignedInteger4 || self.value.len() != 4 {
            return None;
        }
        Some(u32::from_le_bytes(self.value[..4].try_into().ok()?))
    }

    /// Interpret the value as a GUID.
    pub fn as_guid(&self) -> Option<Uuid> {
        if self.ty != PhysicalType::Guid || self.value.len() != 16 {
            return None;
        }
        Some(read_guid(&self.value))
    }

    /// Interpret the value as a 64-bit float.
    pub fn as_f64(&self) -> Option<f64> {
        if self.ty != PhysicalType::Real8 || self.value.len() != 8 {
            return None;
        }
        Some(f64::from_le_bytes(self.value[..8].try_into().ok()?))
    }
}

/// A vector element — an ordered list of values of one physical type.
#[derive(Debug, Clone)]
pub struct VectorElement {
    /// Tag GUID.
    pub tag: Uuid,
    /// Physical type of each element.
    pub ty: PhysicalType,
    /// Raw value bytes: `count * ty.byte_size()`.
    pub values: Vec<u8>,
}

impl VectorElement {
    /// Number of values encoded in `values`.
    pub fn len(&self) -> usize {
        self.values.len() / self.ty.byte_size()
    }

    /// Whether the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// A collection element — a nested element tree.
#[derive(Debug, Clone)]
pub struct CollectionElement {
    /// Tag GUID.
    pub tag: Uuid,
    /// Children in file order.
    pub children: Vec<Element>,
}

impl CollectionElement {
    /// Find the first scalar child with the given tag.
    pub fn scalar(&self, tag: Uuid) -> Option<&ScalarElement> {
        self.children.iter().find_map(|c| match c {
            Element::Scalar(s) if s.tag == tag => Some(s),
            _ => None,
        })
    }

    /// Find the first vector child with the given tag.
    pub fn vector(&self, tag: Uuid) -> Option<&VectorElement> {
        self.children.iter().find_map(|c| match c {
            Element::Vector(v) if v.tag == tag => Some(v),
            _ => None,
        })
    }

    /// Find the first collection child with the given tag.
    pub fn collection(&self, tag: Uuid) -> Option<&CollectionElement> {
        self.children.iter().find_map(|c| match c {
            Element::Collection(c) if c.tag == tag => Some(c),
            _ => None,
        })
    }

    /// Iterate every direct child with the given tag.
    pub fn children_with_tag<'a>(&'a self, tag: Uuid) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.tag() == tag)
    }
}

/// Decode a whole record body into a top-level Collection whose `children`
/// are the record's tags.
///
/// The record body begins with an `Int32` count of top-level element headers,
/// followed by that many contiguous 28-byte element headers. Non-embedded
/// scalars, vectors, and collections resolve their payloads by seeking to
/// `link_offset` within `body`.
pub fn decode_body(body: &[u8]) -> Result<CollectionElement, PqdifError> {
    if body.len() < 4 {
        return Err(PqdifError::Truncated);
    }
    let count = i32::from_le_bytes(body[0..4].try_into().unwrap());
    if count < 0 {
        return Err(PqdifError::BadHeader("negative element count"));
    }
    let count = count as usize;
    let headers_end = 4usize
        .checked_add(
            count
                .checked_mul(ELEMENT_HEADER_LEN)
                .ok_or(PqdifError::Truncated)?,
        )
        .ok_or(PqdifError::Truncated)?;
    if body.len() < headers_end {
        return Err(PqdifError::Truncated);
    }
    let mut children = Vec::with_capacity(count);
    for i in 0..count {
        let start = 4 + i * ELEMENT_HEADER_LEN;
        let header = &body[start..start + ELEMENT_HEADER_LEN];
        children.push(decode_element(header, body)?);
    }
    Ok(CollectionElement {
        // Body-level "collection" is virtual — no tag on the wire.
        tag: Uuid::nil(),
        children,
    })
}

fn decode_element(header: &[u8], body: &[u8]) -> Result<Element, PqdifError> {
    debug_assert_eq!(header.len(), ELEMENT_HEADER_LEN);
    let tag = read_guid(&header[0..16]);
    let element_type = header[16];
    let physical_type_byte = header[17];
    let is_embedded = header[18] != 0;
    let payload = &header[20..28];

    match element_type {
        ELEMENT_TYPE_SCALAR => decode_scalar(tag, physical_type_byte, is_embedded, payload, body),
        ELEMENT_TYPE_VECTOR => decode_vector(tag, physical_type_byte, payload, body),
        ELEMENT_TYPE_COLLECTION => decode_collection(tag, payload, body),
        other => Err(PqdifError::UnknownElementType(other)),
    }
}

fn decode_scalar(
    tag: Uuid,
    physical_type_byte: u8,
    is_embedded: bool,
    payload: &[u8],
    body: &[u8],
) -> Result<Element, PqdifError> {
    let ty = PhysicalType::from_byte(physical_type_byte)?;
    let size = ty.byte_size();
    // The wire's `isEmbedded` byte is authoritative. GSF's writer sets it to 1
    // iff the value fits in the 8-byte payload slot (i.e. `size <= 8`); older
    // GSF writers occasionally embed Real8 (size == 8) values by keeping this
    // byte set, so we follow the flag rather than a size-based heuristic. Non-
    // scalars never set it; their payload is always a `(link, padded_size)`.
    let value = if is_embedded {
        if size > payload.len() {
            return Err(PqdifError::BadHeader(
                "isEmbedded set but physical type does not fit the payload slot",
            ));
        }
        payload[..size].to_vec()
    } else {
        let link_offset = i32::from_le_bytes(payload[0..4].try_into().unwrap());
        let padded_size = i32::from_le_bytes(payload[4..8].try_into().unwrap());
        if link_offset < 0 || padded_size < 0 {
            return Err(PqdifError::BadHeader("negative link offset / padded size"));
        }
        let start = link_offset as usize;
        let end = start.checked_add(size).ok_or(PqdifError::Truncated)?;
        if end > body.len() {
            return Err(PqdifError::Truncated);
        }
        // padded_size is padding, not authoritative — some writers leave it
        // stale — but if it is stored at all it must at least cover the raw
        // size. GSF's writer stores `padSize = ((raw + 3) / 4) * 4`; a value
        // smaller than that is inconsistent enough to reject.
        if (padded_size as usize) < size {
            return Err(PqdifError::BadHeader("padded_size smaller than raw size"));
        }
        body[start..end].to_vec()
    };
    Ok(Element::Scalar(ScalarElement { tag, ty, value }))
}

fn decode_vector(
    tag: Uuid,
    physical_type_byte: u8,
    payload: &[u8],
    body: &[u8],
) -> Result<Element, PqdifError> {
    let ty = PhysicalType::from_byte(physical_type_byte)?;
    let link_offset = i32::from_le_bytes(payload[0..4].try_into().unwrap());
    let padded_size = i32::from_le_bytes(payload[4..8].try_into().unwrap());
    if link_offset < 0 || padded_size < 0 {
        return Err(PqdifError::BadHeader("negative link offset / padded size"));
    }
    let start = link_offset as usize;
    if start.checked_add(4).ok_or(PqdifError::Truncated)? > body.len() {
        return Err(PqdifError::Truncated);
    }
    let count = i32::from_le_bytes(body[start..start + 4].try_into().unwrap());
    if count < 0 {
        return Err(PqdifError::BadHeader("negative vector count"));
    }
    let count = count as usize;
    let raw_size = count
        .checked_mul(ty.byte_size())
        .ok_or(PqdifError::Truncated)?;
    let values_start = start + 4;
    let values_end = values_start
        .checked_add(raw_size)
        .ok_or(PqdifError::Truncated)?;
    if values_end > body.len() {
        return Err(PqdifError::Truncated);
    }
    let values = body[values_start..values_end].to_vec();
    Ok(Element::Vector(VectorElement { tag, ty, values }))
}

fn decode_collection(tag: Uuid, payload: &[u8], body: &[u8]) -> Result<Element, PqdifError> {
    let link_offset = i32::from_le_bytes(payload[0..4].try_into().unwrap());
    let padded_size = i32::from_le_bytes(payload[4..8].try_into().unwrap());
    if link_offset < 0 || padded_size < 0 {
        return Err(PqdifError::BadHeader("negative link offset / padded size"));
    }
    let start = link_offset as usize;
    if start.checked_add(4).ok_or(PqdifError::Truncated)? > body.len() {
        return Err(PqdifError::Truncated);
    }
    let count = i32::from_le_bytes(body[start..start + 4].try_into().unwrap());
    if count < 0 {
        return Err(PqdifError::BadHeader("negative collection count"));
    }
    let count = count as usize;
    let mut children = Vec::with_capacity(count);
    for i in 0..count {
        let hstart = start + 4 + i * ELEMENT_HEADER_LEN;
        let hend = hstart
            .checked_add(ELEMENT_HEADER_LEN)
            .ok_or(PqdifError::Truncated)?;
        if hend > body.len() {
            return Err(PqdifError::Truncated);
        }
        children.push(decode_element(&body[hstart..hend], body)?);
    }
    Ok(Element::Collection(CollectionElement { tag, children }))
}

/// Round `n` up to the nearest multiple of 4 (the padded-size formula).
#[allow(dead_code)]
pub(crate) fn padded_size(n: usize) -> usize {
    (n + 3) & !3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_size_rounds_up_to_four() {
        assert_eq!(padded_size(0), 0);
        assert_eq!(padded_size(1), 4);
        assert_eq!(padded_size(3), 4);
        assert_eq!(padded_size(4), 4);
        assert_eq!(padded_size(5), 8);
        assert_eq!(padded_size(8), 8);
        assert_eq!(padded_size(9), 12);
    }

    #[test]
    fn embedded_boundary_at_size_eight() {
        // Real8 (8 bytes) must be *linked*, not embedded.
        // Real4 (4 bytes) must be *embedded*.
        // Build a tiny body with one embedded Real4 and one linked Real8.
        let real4_tag = uuid::uuid!("11111111-1111-1111-1111-111111111111");
        let real8_tag = uuid::uuid!("22222222-2222-2222-2222-222222222222");

        // Body layout:
        //  0..4:   count = 2
        //  4..32:  header of Real4 (embedded)
        //  32..60: header of Real8 (linked, offset -> 60)
        //  60..68: Real8 value bytes
        let mut body = vec![0u8; 68];
        body[0..4].copy_from_slice(&2i32.to_le_bytes());

        // Real4 header — embedded 3.5_f32. Header lives at body[4..32]; the
        // payload slot inside that header is header[20..28] = body[24..32].
        write_element_header(
            &mut body[4..32],
            real4_tag,
            ELEMENT_TYPE_SCALAR,
            PhysicalType::Real4 as u8,
            true,
        );
        body[24..28].copy_from_slice(&3.5_f32.to_le_bytes());

        // Real8 header — linked at body offset 60, padded_size=8.
        write_element_header(
            &mut body[32..60],
            real8_tag,
            ELEMENT_TYPE_SCALAR,
            PhysicalType::Real8 as u8,
            false,
        );
        body[52..56].copy_from_slice(&60i32.to_le_bytes());
        body[56..60].copy_from_slice(&8i32.to_le_bytes());
        body[60..68].copy_from_slice(&2.5_f64.to_le_bytes());

        let coll = decode_body(&body).unwrap();
        assert_eq!(coll.children.len(), 2);
        match &coll.children[0] {
            Element::Scalar(s) => {
                assert_eq!(s.tag, real4_tag);
                assert_eq!(s.ty, PhysicalType::Real4);
                assert_eq!(s.value.len(), 4);
                assert_eq!(f32::from_le_bytes(s.value[..4].try_into().unwrap()), 3.5);
            }
            _ => panic!("expected scalar"),
        }
        match &coll.children[1] {
            Element::Scalar(s) => {
                assert_eq!(s.tag, real8_tag);
                assert_eq!(s.ty, PhysicalType::Real8);
                assert_eq!(s.as_f64(), Some(2.5));
            }
            _ => panic!("expected scalar"),
        }
    }

    #[test]
    fn vector_layout_int32_count_then_values() {
        let tag = uuid::uuid!("33333333-3333-3333-3333-333333333333");
        // Body layout:
        //  0..4:   count = 1
        //  4..32:  header of Integer4 vector (linked at 32)
        //  32..36: vector count = 3
        //  36..48: three i32 values
        let mut body = vec![0u8; 48];
        body[0..4].copy_from_slice(&1i32.to_le_bytes());
        write_element_header(
            &mut body[4..32],
            tag,
            ELEMENT_TYPE_VECTOR,
            PhysicalType::Integer4 as u8,
            false,
        );
        body[24..28].copy_from_slice(&32i32.to_le_bytes());
        body[28..32].copy_from_slice(&12i32.to_le_bytes());
        body[32..36].copy_from_slice(&3i32.to_le_bytes());
        body[36..40].copy_from_slice(&10i32.to_le_bytes());
        body[40..44].copy_from_slice(&20i32.to_le_bytes());
        body[44..48].copy_from_slice(&30i32.to_le_bytes());

        let coll = decode_body(&body).unwrap();
        match &coll.children[0] {
            Element::Vector(v) => {
                assert_eq!(v.tag, tag);
                assert_eq!(v.ty, PhysicalType::Integer4);
                assert_eq!(v.len(), 3);
                let a = i32::from_le_bytes(v.values[0..4].try_into().unwrap());
                let b = i32::from_le_bytes(v.values[4..8].try_into().unwrap());
                let c = i32::from_le_bytes(v.values[8..12].try_into().unwrap());
                assert_eq!((a, b, c), (10, 20, 30));
            }
            _ => panic!("expected vector"),
        }
    }

    #[test]
    fn truncated_body_rejected() {
        // Count says 1, but the header does not fit.
        let mut body = vec![0u8; 8];
        body[0..4].copy_from_slice(&1i32.to_le_bytes());
        assert!(matches!(decode_body(&body), Err(PqdifError::Truncated)));
    }

    #[test]
    fn unknown_element_type_rejected() {
        let tag = uuid::uuid!("44444444-4444-4444-4444-444444444444");
        let mut body = vec![0u8; 32];
        body[0..4].copy_from_slice(&1i32.to_le_bytes());
        write_element_header(
            &mut body[4..32],
            tag,
            99,
            PhysicalType::Integer4 as u8,
            false,
        );
        assert!(matches!(
            decode_body(&body),
            Err(PqdifError::UnknownElementType(99))
        ));
    }

    /// Test helper — write a 28-byte element header at `out`. `is_embedded`
    /// controls byte 18 and only has any effect for Scalar elements.
    fn write_element_header(
        out: &mut [u8],
        tag: Uuid,
        element_type: u8,
        physical_type: u8,
        is_embedded: bool,
    ) {
        assert_eq!(out.len(), ELEMENT_HEADER_LEN);
        crate::physical::header::write_guid(tag, &mut out[0..16]);
        out[16] = element_type;
        out[17] = physical_type;
        out[18] = if is_embedded { 1 } else { 0 };
        out[19] = 0;
        // payload cleared by caller.
    }
}
