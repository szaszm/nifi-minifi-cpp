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

//! Hand-built PQDIF byte array: Container + DataSource + Observation.
//!
//! Serialization is a two-phase walk over an intermediate element tree
//! (`FElem`). Phase 1 assigns every element a byte position inside the record
//! body and computes the record body's total size; phase 2 writes the bytes
//! with every `link_offset` resolved absolute to the record body root, which
//! is what the reader expects.

use uuid::Uuid;

/// GUID from `RECORD_SIGNATURE` in the crate (repeated verbatim so the
/// fixture is self-contained).
const SIGNATURE: Uuid = uuid::uuid!("4b1c9187-e88e-11cf-9d89-0080c72e70a3");

// Record-level GUIDs.
const CONTAINER_TAG: Uuid = uuid::uuid!("89738606-f1c3-11cf-9d89-0080c72e70a3");
const DATA_SOURCE_TAG: Uuid = uuid::uuid!("89738619-f1c3-11cf-9d89-0080c72e70a3");
const OBSERVATION_TAG: Uuid = uuid::uuid!("8973861a-f1c3-11cf-9d89-0080c72e70a3");

// Container inner tags.
const VERSION_INFO_TAG: Uuid = uuid::uuid!("89738607-f1c3-11cf-9d89-0080c72e70a3");
const APPLICATION_TAG: Uuid = uuid::uuid!("89738623-f1c3-11cf-9d89-0080c72e70a3");
const CREATION_TAG: Uuid = uuid::uuid!("89738609-f1c3-11cf-9d89-0080c72e70a3");

// DataSource / channel-definition tags.
const DATA_SOURCE_NAME_TAG: Uuid = uuid::uuid!("b48d8587-f5f5-11cf-9d89-0080c72e70a3");
const CHANNEL_DEFINITIONS_TAG: Uuid = uuid::uuid!("b48d858d-f5f5-11cf-9d89-0080c72e70a3");
const ONE_CHANNEL_DEFINITION_TAG: Uuid = uuid::uuid!("b48d858e-f5f5-11cf-9d89-0080c72e70a3");
const CHANNEL_NAME_TAG: Uuid = uuid::uuid!("b48d8590-f5f5-11cf-9d89-0080c72e70a3");
const CHANNEL_DEFINITION_INDEX_TAG: Uuid = uuid::uuid!("b48d858f-f5f5-11cf-9d89-0080c72e70a3");

// Observation-level tags.
const OBSERVATION_NAME_TAG: Uuid = uuid::uuid!("3d786f8a-f76e-11cf-9d89-0080c72e70a3");
const TIME_START_TAG: Uuid = uuid::uuid!("3d786f8c-f76e-11cf-9d89-0080c72e70a3");
const CHANNEL_INSTANCES_TAG: Uuid = uuid::uuid!("3d786f91-f76e-11cf-9d89-0080c72e70a3");
const ONE_CHANNEL_INSTANCE_TAG: Uuid = uuid::uuid!("3d786f92-f76e-11cf-9d89-0080c72e70a3");
const SERIES_INSTANCES_TAG: Uuid = uuid::uuid!("3d786f93-f76e-11cf-9d89-0080c72e70a3");
const ONE_SERIES_INSTANCE_TAG: Uuid = uuid::uuid!("3d786f94-f76e-11cf-9d89-0080c72e70a3");
const SERIES_VALUES_TAG: Uuid = uuid::uuid!("3d786f99-f76e-11cf-9d89-0080c72e70a3");

// Physical types (byte discriminants).
const PT_UNSIGNED_INTEGER_4: u8 = 32;
const PT_REAL_8: u8 = 41;
const PT_TIMESTAMP: u8 = 50;
const PT_CHAR_1: u8 = 10;

// Element types.
const ET_SCALAR: u8 = 1;
const ET_VECTOR: u8 = 2;
const ET_COLLECTION: u8 = 3;

/// Build a valid PQDIF byte array with one Container, one DataSource, and
/// one Observation. Returns the file bytes plus the sample series that were
/// encoded inside the Observation (for the smoke tests to compare against).
pub fn build() -> (Vec<u8>, Vec<f64>) {
    let container = container_body();
    let data_source = data_source_body();
    let samples = vec![10.0_f64, 20.0, 30.0, 40.0, 50.0];
    let observation = observation_body(&samples);

    const HEADER_LEN: usize = 64;
    let container_len = HEADER_LEN + container.len();
    let data_source_offset = container_len;
    let data_source_len = HEADER_LEN + data_source.len();
    let observation_offset = data_source_offset + data_source_len;

    let mut out = Vec::new();
    write_record(&mut out, CONTAINER_TAG, data_source_offset as u32, &container);
    write_record(&mut out, DATA_SOURCE_TAG, observation_offset as u32, &data_source);
    write_record(&mut out, OBSERVATION_TAG, 0, &observation);

    (out, samples)
}

/// Write a record header + body at the end of `out`.
fn write_record(out: &mut Vec<u8>, type_tag: Uuid, next_record_pos: u32, body: &[u8]) {
    let mut header = [0u8; 64];
    write_guid(&mut header[0..16], SIGNATURE);
    write_guid(&mut header[16..32], type_tag);
    header[32..36].copy_from_slice(&64u32.to_le_bytes());
    header[36..40].copy_from_slice(&(body.len() as u32).to_le_bytes());
    header[40..44].copy_from_slice(&next_record_pos.to_le_bytes());
    // 44..60 zero-filled.
    let checksum = adler32(body);
    header[60..64].copy_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(&header);
    out.extend_from_slice(body);
}

// ---------- Element tree used during construction ----------

/// Intermediate element description for the fixture builder. Every variant
/// carries the fields the on-disk element header needs.
enum FElem {
    /// Embedded scalar (physical-type byte size < 8): value bytes live in
    /// the header's payload slot.
    ScalarEmbedded {
        tag: Uuid,
        physical_type: u8,
        /// 4-byte little-endian representation of the value.
        embedded: [u8; 4],
    },
    /// Linked scalar (physical-type byte size >= 8): value bytes live in the
    /// record body pool at the resolved link.
    ScalarLinked {
        tag: Uuid,
        physical_type: u8,
        payload: Vec<u8>,
    },
    Vector {
        tag: Uuid,
        physical_type: u8,
        count: u32,
        /// Raw value bytes — count * physical-type byte size.
        raw: Vec<u8>,
    },
    Collection {
        tag: Uuid,
        children: Vec<FElem>,
    },
}

// ---------- Body builders ----------

fn container_body() -> Vec<u8> {
    let mut children = Vec::new();
    // Scalar UnsignedInteger4 embedded, value = 1.
    children.push(FElem::ScalarEmbedded {
        tag: VERSION_INFO_TAG,
        physical_type: PT_UNSIGNED_INTEGER_4,
        embedded: 1u32.to_le_bytes(),
    });
    children.push(vector_char1(APPLICATION_TAG, b"pqdif-rs\0"));
    // Creation timestamp: 2020-06-15 12:00:00 UTC.
    // days = 44000 raw; parser subtracts 2 to get 2020-06-15.
    let mut ts_payload = Vec::with_capacity(12);
    ts_payload.extend_from_slice(&44000u32.to_le_bytes());
    ts_payload.extend_from_slice(&43200.0_f64.to_le_bytes());
    children.push(FElem::ScalarLinked {
        tag: CREATION_TAG,
        physical_type: PT_TIMESTAMP,
        payload: ts_payload,
    });
    serialize_body(&children)
}

fn data_source_body() -> Vec<u8> {
    let mut children = Vec::new();
    children.push(vector_char1(DATA_SOURCE_NAME_TAG, b"synthetic\0"));

    let one_channel_definition = FElem::Collection {
        tag: ONE_CHANNEL_DEFINITION_TAG,
        children: vec![vector_char1(CHANNEL_NAME_TAG, b"CH1\0")],
    };
    children.push(FElem::Collection {
        tag: CHANNEL_DEFINITIONS_TAG,
        children: vec![one_channel_definition],
    });
    serialize_body(&children)
}

fn observation_body(samples: &[f64]) -> Vec<u8> {
    let mut children = Vec::new();
    children.push(vector_char1(OBSERVATION_NAME_TAG, b"OBS1\0"));

    let mut ts_payload = Vec::with_capacity(12);
    ts_payload.extend_from_slice(&44000u32.to_le_bytes());
    ts_payload.extend_from_slice(&43260.0_f64.to_le_bytes());
    children.push(FElem::ScalarLinked {
        tag: TIME_START_TAG,
        physical_type: PT_TIMESTAMP,
        payload: ts_payload,
    });

    // One series-instance: an explicit `Real8` vector of the samples.
    let mut sample_bytes = Vec::with_capacity(samples.len() * 8);
    for v in samples {
        sample_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let one_series_instance = FElem::Collection {
        tag: ONE_SERIES_INSTANCE_TAG,
        children: vec![FElem::Vector {
            tag: SERIES_VALUES_TAG,
            physical_type: PT_REAL_8,
            count: samples.len() as u32,
            raw: sample_bytes,
        }],
    };

    let one_channel_instance = FElem::Collection {
        tag: ONE_CHANNEL_INSTANCE_TAG,
        children: vec![
            FElem::ScalarEmbedded {
                tag: CHANNEL_DEFINITION_INDEX_TAG,
                physical_type: PT_UNSIGNED_INTEGER_4,
                embedded: 0u32.to_le_bytes(),
            },
            FElem::Collection {
                tag: SERIES_INSTANCES_TAG,
                children: vec![one_series_instance],
            },
        ],
    };

    children.push(FElem::Collection {
        tag: CHANNEL_INSTANCES_TAG,
        children: vec![one_channel_instance],
    });
    serialize_body(&children)
}

fn vector_char1(tag: Uuid, bytes: &[u8]) -> FElem {
    FElem::Vector {
        tag,
        physical_type: PT_CHAR_1,
        count: bytes.len() as u32,
        raw: bytes.to_vec(),
    }
}

// ---------- Two-phase serializer ----------

/// Phase 1: walk the tree, allocate space, and record every element header's
/// final byte position in the record body. Phase 2: emit bytes with all
/// `link_offset` fields resolved to absolute body positions.
///
/// A body is laid out as:
///   `Int32 count` | `count` × 28-byte element headers | pool
///
/// Every non-embedded element's link points into the record body root (not
/// into any nested collection payload). This is what the reader expects.
fn serialize_body(children: &[FElem]) -> Vec<u8> {
    let mut out = vec![0u8; 4 + children.len() * 28];
    // Count at offset 0.
    out[0..4].copy_from_slice(&(children.len() as i32).to_le_bytes());
    for (i, child) in children.iter().enumerate() {
        let hstart = 4 + i * 28;
        write_element(&mut out, hstart, child);
    }
    out
}

/// Write one element: fill in the 28-byte header at `header_pos` (already
/// reserved in `out`), and append any pool payload to `out`, patching the
/// header's link_offset with the appended position.
fn write_element(out: &mut Vec<u8>, header_pos: usize, elem: &FElem) {
    match elem {
        FElem::ScalarEmbedded {
            tag,
            physical_type,
            embedded,
        } => {
            let mut h = [0u8; 28];
            write_guid(&mut h[0..16], *tag);
            h[16] = ET_SCALAR;
            h[17] = *physical_type;
            h[20..24].copy_from_slice(embedded);
            // 24..28 zero (padding).
            out[header_pos..header_pos + 28].copy_from_slice(&h);
        }
        FElem::ScalarLinked {
            tag,
            physical_type,
            payload,
        } => {
            let (link, padded) = append_payload(out, payload);
            let mut h = [0u8; 28];
            write_guid(&mut h[0..16], *tag);
            h[16] = ET_SCALAR;
            h[17] = *physical_type;
            h[20..24].copy_from_slice(&link.to_le_bytes());
            h[24..28].copy_from_slice(&padded.to_le_bytes());
            out[header_pos..header_pos + 28].copy_from_slice(&h);
        }
        FElem::Vector {
            tag,
            physical_type,
            count,
            raw,
        } => {
            let mut payload = Vec::with_capacity(4 + raw.len());
            payload.extend_from_slice(&count.to_le_bytes());
            payload.extend_from_slice(raw);
            let (link, padded) = append_payload(out, &payload);
            let mut h = [0u8; 28];
            write_guid(&mut h[0..16], *tag);
            h[16] = ET_VECTOR;
            h[17] = *physical_type;
            h[20..24].copy_from_slice(&link.to_le_bytes());
            h[24..28].copy_from_slice(&padded.to_le_bytes());
            out[header_pos..header_pos + 28].copy_from_slice(&h);
        }
        FElem::Collection { tag, children } => {
            // Reserve the collection's own header block in the pool: 4-byte
            // count + 28-byte per child. Then recursively write each child
            // into that reserved header area, letting them append their
            // payloads to the same pool.
            let coll_start = out.len();
            let coll_header_block = 4 + children.len() * 28;
            out.resize(coll_start + coll_header_block, 0);
            out[coll_start..coll_start + 4]
                .copy_from_slice(&(children.len() as i32).to_le_bytes());
            for (i, child) in children.iter().enumerate() {
                let child_header = coll_start + 4 + i * 28;
                write_element(out, child_header, child);
            }
            // Pad the collection block itself to 4-byte alignment (already is,
            // since header block is a multiple of 4). Compute the collection's
            // total size on-disk: from `coll_start` to the end of everything
            // its children appended.
            let coll_end = out.len();
            let raw_len = coll_end - coll_start;
            let padded = (raw_len + 3) & !3;
            out.resize(coll_start + padded, 0);

            let mut h = [0u8; 28];
            write_guid(&mut h[0..16], *tag);
            h[16] = ET_COLLECTION;
            h[17] = 0; // no physical type for collections
            h[20..24].copy_from_slice(&(coll_start as i32).to_le_bytes());
            h[24..28].copy_from_slice(&(padded as i32).to_le_bytes());
            out[header_pos..header_pos + 28].copy_from_slice(&h);
        }
    }
}

/// Append `payload` to `out`, pad it to a 4-byte multiple, and return
/// `(link_offset, padded_size)` — both `i32` for direct copy into a header.
fn append_payload(out: &mut Vec<u8>, payload: &[u8]) -> (i32, i32) {
    let start = out.len();
    out.extend_from_slice(payload);
    let raw_size = payload.len();
    let padded_size = (raw_size + 3) & !3;
    out.resize(start + padded_size, 0);
    (start as i32, padded_size as i32)
}

/// Write a 16-byte GUID matching the on-disk encoding (see
/// `physical::header::write_guid`).
fn write_guid(out: &mut [u8], guid: Uuid) {
    let (a, b, c, d) = guid.as_fields();
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..6].copy_from_slice(&b.to_le_bytes());
    out[6..8].copy_from_slice(&c.to_le_bytes());
    out[8..16].copy_from_slice(d);
}

/// Adler-32 (RFC 1950). Duplicates `physical::checksum::adler32` so the
/// fixture stays independent of the crate internals.
fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}
