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

//! Hand-built PQDIF byte arrays used by this crate's integration tests and by
//! dependents that enable the `test-fixtures` feature.
//!
//! [`build`] emits the canonical Container + DataSource + Observation triple
//! the smoke tests started with; the family of `build_*` helpers alongside it
//! covers scale/offset, compact `(start, count, increment)` encoding,
//! multi-observation chains, spec-violating shapes the parser is expected to
//! tolerate (second Container, Blank record), fatal shapes (cycle,
//! non-Container first record), UTF-16 strings, and zlib-compressed record
//! bodies.
//!
//! Serialization is a two-phase walk over an intermediate element tree
//! (`FElem`). Phase 1 assigns every element a byte position inside the record
//! body and computes the record body's total size; phase 2 writes the bytes
//! with every `link_offset` resolved absolute to the record body root, which
//! is what the reader expects.

use uuid::Uuid;

/// GUID from `RECORD_SIGNATURE` in the crate (repeated verbatim so the
/// fixture is self-contained).
const SIGNATURE: Uuid = uuid::uuid!("4a111440-e49f-11cf-9900-505144494600");

// Record-level GUIDs.
const CONTAINER_TAG: Uuid = uuid::uuid!("89738606-f1c3-11cf-9d89-0080c72e70a3");
const DATA_SOURCE_TAG: Uuid = uuid::uuid!("89738619-f1c3-11cf-9d89-0080c72e70a3");
const OBSERVATION_TAG: Uuid = uuid::uuid!("8973861a-f1c3-11cf-9d89-0080c72e70a3");
const BLANK_TAG: Uuid = uuid::uuid!("89738618-f1c3-11cf-9d89-0080c72e70a3");

// Container inner tags.
const VERSION_INFO_TAG: Uuid = uuid::uuid!("89738607-f1c3-11cf-9d89-0080c72e70a3");
const APPLICATION_TAG: Uuid = uuid::uuid!("89738623-f1c3-11cf-9d89-0080c72e70a3");
const CREATION_TAG: Uuid = uuid::uuid!("89738609-f1c3-11cf-9d89-0080c72e70a3");
const COMPRESSION_STYLE_TAG: Uuid = uuid::uuid!("8973861b-f1c3-11cf-9d89-0080c72e70a3");
const COMPRESSION_ALGORITHM_TAG: Uuid = uuid::uuid!("8973861c-f1c3-11cf-9d89-0080c72e70a3");
// The remaining optional Container tags, emitted only by
// `build_with_all_container_metadata`.
const FILE_NAME_TAG: Uuid = uuid::uuid!("89738608-f1c3-11cf-9d89-0080c72e70a3");
const LAST_SAVED_TAG: Uuid = uuid::uuid!("8973860a-f1c3-11cf-9d89-0080c72e70a3");
const TIMES_SAVED_TAG: Uuid = uuid::uuid!("8973860b-f1c3-11cf-9d89-0080c72e70a3");
const TITLE_TAG: Uuid = uuid::uuid!("8973860d-f1c3-11cf-9d89-0080c72e70a3");
const AUTHOR_TAG: Uuid = uuid::uuid!("8973860f-f1c3-11cf-9d89-0080c72e70a3");
const NOTES_TAG: Uuid = uuid::uuid!("89738617-f1c3-11cf-9d89-0080c72e70a3");

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
const SERIES_SCALE_TAG: Uuid = uuid::uuid!("3d786f96-f76e-11cf-9d89-0080c72e70a3");
const SERIES_OFFSET_TAG: Uuid = uuid::uuid!("3d786f97-f76e-11cf-9d89-0080c72e70a3");
const SERIES_VALUES_TAG: Uuid = uuid::uuid!("3d786f99-f76e-11cf-9d89-0080c72e70a3");
const STORAGE_METHOD_ID_TAG: Uuid = uuid::uuid!("b48d85a1-f5f5-11cf-9d89-0080c72e70a3");

// Physical types (byte discriminants).
const PT_UNSIGNED_INTEGER_4: u8 = 32;
const PT_REAL_8: u8 = 41;
const PT_TIMESTAMP: u8 = 50;
const PT_CHAR_1: u8 = 10;
const PT_CHAR_2: u8 = 11;

// Element types.
// Match the reference `GSF.PQDIF/Physical/Element.cs` enum values:
// Collection = 1, Scalar = 2, Vector = 3.
const ET_COLLECTION: u8 = 1;
const ET_SCALAR: u8 = 2;
const ET_VECTOR: u8 = 3;

// Compression enumeration values (from tag_definitions.xml).
const ID_COMP_STYLE_RECORDLEVEL: u32 = 2;
const ID_COMP_ALG_ZLIB: u32 = 1;

// Storage-method bitmask values.
const STORAGE_METHOD_INCREMENT: u32 = 0x04;

const HEADER_LEN: usize = 64;

/// Build the canonical single-observation fixture. Returns the file bytes plus
/// the sample series that were encoded inside the Observation (for smoke tests
/// to compare against).
pub fn build() -> (Vec<u8>, Vec<f64>) {
    let samples = vec![10.0_f64, 20.0, 30.0, 40.0, 50.0];
    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let observation = observation_body(
        "OBS1",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: samples.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: observation,
        },
    ]);
    (bytes, samples)
}

/// Container + DataSource + N Observations, each with the same shape as
/// [`build`] but a unique name. Returns the file bytes and the sample vector
/// each observation carries.
pub fn build_multi_observation(n: usize) -> (Vec<u8>, Vec<Vec<f64>>) {
    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);

    let mut samples_out = Vec::with_capacity(n);
    let mut records = vec![
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
    ];
    for i in 0..n {
        let base = (i as f64) * 100.0;
        let samples: Vec<f64> = (0..3).map(|k| base + (k as f64 + 1.0)).collect();
        samples_out.push(samples.clone());
        let obs = observation_body(
            &format!("OBS{}", i + 1),
            &[ChannelInstanceSpec {
                channel_definition_index: 0,
                series: vec![SeriesSpec::Explicit {
                    samples,
                    scale: None,
                    offset: None,
                }],
            }],
        );
        records.push(RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs,
        });
    }
    (assemble_chain(&records), samples_out)
}

/// Fixture with `tagSeriesScale` and `tagSeriesOffset` scalars set. Returns
/// the file bytes plus the *decoded* values callers should expect (i.e. after
/// `value * scale + offset` has been applied).
pub fn build_with_scale_offset() -> (Vec<u8>, Vec<f64>) {
    let raw = vec![1.0_f64, 2.0, 3.0];
    let scale = 10.0_f64;
    let offset = 100.0_f64;
    let expected: Vec<f64> = raw.iter().map(|v| v * scale + offset).collect();

    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let observation = observation_body(
        "OBS_SCALED",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: raw,
                scale: Some(scale),
                offset: Some(offset),
            }],
        }],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: observation,
        },
    ]);
    (bytes, expected)
}

/// Fixture using compact `(start, count, increment)` series encoding. Returns
/// the file bytes plus the expanded values the parser should produce.
pub fn build_with_compact_series() -> (Vec<u8>, Vec<f64>) {
    let start = 5.0_f64;
    let count = 4_u32;
    let increment = 2.0_f64;
    let expected: Vec<f64> = (0..count).map(|i| start + increment * i as f64).collect();

    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let observation = observation_body(
        "OBS_COMPACT",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Compact {
                start,
                count,
                increment,
            }],
        }],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: observation,
        },
    ]);
    (bytes, expected)
}

/// Container + DataSource + Observation + Blank + Observation. `next_observation`
/// is expected to skip the Blank record silently.
pub fn build_with_blank_record() -> (Vec<u8>, [Vec<f64>; 2]) {
    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let s0 = vec![1.0, 2.0, 3.0];
    let s1 = vec![4.0, 5.0, 6.0];
    let obs0 = observation_body(
        "OBS_A",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: s0.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    let obs1 = observation_body(
        "OBS_B",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: s1.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    // A Blank record needs a body — an empty element list (count = 0).
    let blank_body = serialize_body(&[]);
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs0,
        },
        RecordSpec {
            type_tag: BLANK_TAG,
            body: blank_body,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs1,
        },
    ]);
    (bytes, [s0, s1])
}

/// Container + DataSource and nothing else — a legal PQDIF file that declares
/// what it *would* record but carries no Observation. Real acquisition software
/// produces these when a device is configured but has not yet triggered, so
/// consumers have to handle "zero observations" as a success rather than an
/// error.
pub fn build_container_and_data_source_only() -> Vec<u8> {
    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
    ])
}

/// A file whose Container populates every optional field the typed
/// [`crate::ContainerRecord`] exposes.
///
/// The canonical [`build`] fixture deliberately leaves most of them unset, to
/// exercise the sparseness real files exhibit. This one is its complement: it
/// lets a dependent assert that nothing is dropped on the way out, which a
/// sparse fixture cannot distinguish from a field that was never read.
pub fn build_with_all_container_metadata() -> Vec<u8> {
    let container = container_body(ContainerOpts {
        full_metadata: true,
        ..ContainerOpts::default()
    });
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let observation = observation_body(
        "OBS1",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: vec![1.0, 2.0],
                scale: None,
                offset: None,
            }],
        }],
    );
    assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: observation,
        },
    ])
}

/// Two Container records back-to-back. The reference parser silently ignores
/// the second one (spec violation, but not fatal).
pub fn build_with_second_container() -> (Vec<u8>, Vec<f64>) {
    let container = container_body(ContainerOpts::default());
    let container2 = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let samples = vec![7.0, 8.0, 9.0];
    let obs = observation_body(
        "OBS1",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: samples.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container2,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs,
        },
    ]);
    (bytes, samples)
}

/// Container + Observation + Container + DataSource + Observation, where a
/// second DataSource swaps the channel definitions mid-file. The two
/// observations therefore reference two different DataSource records.
pub fn build_data_source_swap() -> (Vec<u8>, [Vec<f64>; 2]) {
    let container = container_body(ContainerOpts::default());
    let ds_a = data_source_body(&[ChannelDefSpec { name: "CH_A" }]);
    let ds_b = data_source_body(&[ChannelDefSpec { name: "CH_B" }]);
    let s0 = vec![1.0];
    let s1 = vec![2.0];
    let obs0 = observation_body(
        "OBS_A",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: s0.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    let obs1 = observation_body(
        "OBS_B",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: s1.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: ds_a,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs0,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: ds_b,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs1,
        },
    ]);
    (bytes, [s0, s1])
}

/// A two-record chain whose second record points back at the first. Triggers
/// `PqdifError::RecordCycle` on the third pull. `next_record_pos == 0` on the
/// header is the sentinel for "no successor", so a genuine cycle needs at
/// least two records.
pub fn build_with_cycle() -> Vec<u8> {
    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let container_len = HEADER_LEN + container.len();
    let data_source_offset = container_len as u32;

    let mut out = Vec::new();
    // First record links forward to the second.
    write_record(&mut out, CONTAINER_TAG, data_source_offset, &container);
    // Second record is written with a placeholder `next_record_pos`, patched
    // below to point back at its own offset — an offset the parser will have
    // already marked visited by the time it follows the link.
    write_record(&mut out, DATA_SOURCE_TAG, 0, &data_source);
    let second_header_start = container_len;
    // next_record_pos lives at bytes [40..44] of the 64-byte header.
    out[second_header_start + 40..second_header_start + 44]
        .copy_from_slice(&data_source_offset.to_le_bytes());
    // The stored checksum covers the body, not the header, so patching
    // next_record_pos leaves it valid.
    out
}

/// A file whose first record is a DataSource (spec violation — first record
/// must be Container). `LogicalParser::open` should reject it.
pub fn build_non_container_first() -> Vec<u8> {
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let mut out = Vec::new();
    write_record(&mut out, DATA_SOURCE_TAG, 0, &data_source);
    out
}

/// Container with `tagApplication` stored as a `Char2` (UTF-16LE) vector.
/// Exercises the alternate string encoding path in `util::decode_string`.
pub fn build_with_utf16_application() -> (Vec<u8>, String) {
    let app = "ünïcode-🔧".to_string();
    let container = container_body(ContainerOpts {
        application_char2: Some(app.clone()),
        ..Default::default()
    });
    let data_source = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let obs = observation_body(
        "OBS1",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: vec![1.0],
                scale: None,
                offset: None,
            }],
        }],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs,
        },
    ]);
    (bytes, app)
}

/// Container declares RecordLevel + Zlib compression; the DataSource and
/// Observation bodies are zlib-encoded (RFC 1950).
pub fn build_zlib_compressed() -> (Vec<u8>, Vec<f64>) {
    let container = container_body(ContainerOpts {
        compression_style: Some(ID_COMP_STYLE_RECORDLEVEL),
        compression_algorithm: Some(ID_COMP_ALG_ZLIB),
        ..Default::default()
    });
    let ds_raw = data_source_body(&[ChannelDefSpec { name: "CH1" }]);
    let samples = vec![11.0_f64, 22.0, 33.0];
    let obs_raw = observation_body(
        "OBS1",
        &[ChannelInstanceSpec {
            channel_definition_index: 0,
            series: vec![SeriesSpec::Explicit {
                samples: samples.clone(),
                scale: None,
                offset: None,
            }],
        }],
    );
    let ds_compressed = zlib_compress(&ds_raw);
    let obs_compressed = zlib_compress(&obs_raw);
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: ds_compressed,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs_compressed,
        },
    ]);
    (bytes, samples)
}

/// Two channels in one Observation. Each channel has one Real8 series with a
/// distinct name. Returns the file bytes plus the per-channel expected samples.
pub fn build_two_channels() -> (Vec<u8>, [Vec<f64>; 2]) {
    let container = container_body(ContainerOpts::default());
    let data_source = data_source_body(&[
        ChannelDefSpec { name: "CH_LEFT" },
        ChannelDefSpec { name: "CH_RIGHT" },
    ]);
    let s0 = vec![1.0, 2.0];
    let s1 = vec![10.0, 20.0, 30.0];
    let obs = observation_body(
        "OBS1",
        &[
            ChannelInstanceSpec {
                channel_definition_index: 0,
                series: vec![SeriesSpec::Explicit {
                    samples: s0.clone(),
                    scale: None,
                    offset: None,
                }],
            },
            ChannelInstanceSpec {
                channel_definition_index: 1,
                series: vec![SeriesSpec::Explicit {
                    samples: s1.clone(),
                    scale: None,
                    offset: None,
                }],
            },
        ],
    );
    let bytes = assemble_chain(&[
        RecordSpec {
            type_tag: CONTAINER_TAG,
            body: container,
        },
        RecordSpec {
            type_tag: DATA_SOURCE_TAG,
            body: data_source,
        },
        RecordSpec {
            type_tag: OBSERVATION_TAG,
            body: obs,
        },
    ]);
    (bytes, [s0, s1])
}

// ---------- Assembly helpers ----------

struct RecordSpec {
    type_tag: Uuid,
    body: Vec<u8>,
}

/// Serialize a chain of records, filling in every header's `next_record_pos`
/// so it points at the next record's absolute file offset (0 for the last).
fn assemble_chain(records: &[RecordSpec]) -> Vec<u8> {
    // Pre-compute offsets so each record's next_record_pos is correct.
    let mut offsets = Vec::with_capacity(records.len() + 1);
    let mut cursor = 0usize;
    for r in records {
        offsets.push(cursor);
        cursor += HEADER_LEN + r.body.len();
    }
    offsets.push(0); // sentinel: last record's "next" is 0

    let mut out = Vec::with_capacity(cursor);
    for (i, r) in records.iter().enumerate() {
        let next = if i + 1 == records.len() {
            0
        } else {
            offsets[i + 1] as u32
        };
        write_record(&mut out, r.type_tag, next, &r.body);
    }
    out
}

/// Write a record header + body at the end of `out`.
fn write_record(out: &mut Vec<u8>, type_tag: Uuid, next_record_pos: u32, body: &[u8]) {
    let mut header = [0u8; HEADER_LEN];
    write_guid(&mut header[0..16], SIGNATURE);
    write_guid(&mut header[16..32], type_tag);
    header[32..36].copy_from_slice(&(HEADER_LEN as u32).to_le_bytes());
    header[36..40].copy_from_slice(&(body.len() as u32).to_le_bytes());
    header[40..44].copy_from_slice(&next_record_pos.to_le_bytes());
    // 44..60 zero-filled.
    let checksum = adler32(body);
    header[44..48].copy_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(&header);
    out.extend_from_slice(body);
}

// ---------- Body-building configuration ----------

#[derive(Default)]
struct ContainerOpts {
    /// If set, wire the `tagApplication` as a `Char2` (UTF-16LE) vector
    /// carrying this string. When `None`, a `Char1` vector "pqdif-rs" is used.
    application_char2: Option<String>,
    /// Optional `tagCompressionStyleID` scalar value.
    compression_style: Option<u32>,
    /// Optional `tagCompressionAlgorithmID` scalar value.
    compression_algorithm: Option<u32>,
    /// Also emit every optional Container tag the typed record exposes —
    /// author, filename, title, notes, last-saved, times-saved.
    full_metadata: bool,
}

struct ChannelDefSpec {
    name: &'static str,
}

struct ChannelInstanceSpec {
    channel_definition_index: u32,
    series: Vec<SeriesSpec>,
}

enum SeriesSpec {
    /// One explicit Real8 vector; optional per-series scale/offset scalars.
    Explicit {
        samples: Vec<f64>,
        scale: Option<f64>,
        offset: Option<f64>,
    },
    /// Compact (start, count, increment) encoding: emits a length-3 Real8
    /// vector plus `tagStorageMethodID = 0x04`.
    Compact {
        start: f64,
        count: u32,
        increment: f64,
    },
}

// ---------- Body builders ----------

fn container_body(opts: ContainerOpts) -> Vec<u8> {
    let mut children = Vec::new();
    // Scalar UnsignedInteger4 embedded, value = 1.
    children.push(FElem::ScalarEmbedded {
        tag: VERSION_INFO_TAG,
        physical_type: PT_UNSIGNED_INTEGER_4,
        embedded: 1u32.to_le_bytes(),
    });
    if let Some(s) = opts.application_char2 {
        children.push(vector_char2(APPLICATION_TAG, &s));
    } else {
        children.push(vector_char1(APPLICATION_TAG, b"pqdif-rs\0"));
    }
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
    if let Some(v) = opts.compression_style {
        children.push(FElem::ScalarEmbedded {
            tag: COMPRESSION_STYLE_TAG,
            physical_type: PT_UNSIGNED_INTEGER_4,
            embedded: v.to_le_bytes(),
        });
    }
    if let Some(v) = opts.compression_algorithm {
        children.push(FElem::ScalarEmbedded {
            tag: COMPRESSION_ALGORITHM_TAG,
            physical_type: PT_UNSIGNED_INTEGER_4,
            embedded: v.to_le_bytes(),
        });
    }
    if opts.full_metadata {
        children.push(vector_char1(AUTHOR_TAG, b"A. Tester\0"));
        children.push(vector_char1(FILE_NAME_TAG, b"recording.pqd\0"));
        children.push(vector_char1(TITLE_TAG, b"Synthetic recording\0"));
        children.push(vector_char1(NOTES_TAG, b"Built by the test fixture\0"));
        // 2020-06-16 12:00:00, a day after the creation timestamp above.
        let mut last_saved = Vec::with_capacity(12);
        last_saved.extend_from_slice(&44001u32.to_le_bytes());
        last_saved.extend_from_slice(&43200.0_f64.to_le_bytes());
        children.push(FElem::ScalarLinked {
            tag: LAST_SAVED_TAG,
            physical_type: PT_TIMESTAMP,
            payload: last_saved,
        });
        children.push(FElem::ScalarEmbedded {
            tag: TIMES_SAVED_TAG,
            physical_type: PT_UNSIGNED_INTEGER_4,
            embedded: 3u32.to_le_bytes(),
        });
    }
    serialize_body(&children)
}

fn data_source_body(channels: &[ChannelDefSpec]) -> Vec<u8> {
    let mut children = Vec::new();
    children.push(vector_char1(DATA_SOURCE_NAME_TAG, b"synthetic\0"));

    let channel_defs: Vec<FElem> = channels
        .iter()
        .map(|c| {
            let mut name = c.name.as_bytes().to_vec();
            name.push(0);
            FElem::Collection {
                tag: ONE_CHANNEL_DEFINITION_TAG,
                children: vec![FElem::Vector {
                    tag: CHANNEL_NAME_TAG,
                    physical_type: PT_CHAR_1,
                    count: name.len() as u32,
                    raw: name,
                }],
            }
        })
        .collect();
    children.push(FElem::Collection {
        tag: CHANNEL_DEFINITIONS_TAG,
        children: channel_defs,
    });
    serialize_body(&children)
}

fn observation_body(name: &str, channels: &[ChannelInstanceSpec]) -> Vec<u8> {
    let mut children = Vec::new();
    let mut name_bytes = name.as_bytes().to_vec();
    name_bytes.push(0);
    children.push(FElem::Vector {
        tag: OBSERVATION_NAME_TAG,
        physical_type: PT_CHAR_1,
        count: name_bytes.len() as u32,
        raw: name_bytes,
    });

    let mut ts_payload = Vec::with_capacity(12);
    ts_payload.extend_from_slice(&44000u32.to_le_bytes());
    ts_payload.extend_from_slice(&43260.0_f64.to_le_bytes());
    children.push(FElem::ScalarLinked {
        tag: TIME_START_TAG,
        physical_type: PT_TIMESTAMP,
        payload: ts_payload,
    });

    let channel_insts: Vec<FElem> = channels
        .iter()
        .map(|c| {
            let series_insts: Vec<FElem> = c.series.iter().map(series_instance).collect();
            FElem::Collection {
                tag: ONE_CHANNEL_INSTANCE_TAG,
                children: vec![
                    FElem::ScalarEmbedded {
                        tag: CHANNEL_DEFINITION_INDEX_TAG,
                        physical_type: PT_UNSIGNED_INTEGER_4,
                        embedded: c.channel_definition_index.to_le_bytes(),
                    },
                    FElem::Collection {
                        tag: SERIES_INSTANCES_TAG,
                        children: series_insts,
                    },
                ],
            }
        })
        .collect();

    children.push(FElem::Collection {
        tag: CHANNEL_INSTANCES_TAG,
        children: channel_insts,
    });
    serialize_body(&children)
}

fn series_instance(spec: &SeriesSpec) -> FElem {
    match spec {
        SeriesSpec::Explicit {
            samples,
            scale,
            offset,
        } => {
            let mut sample_bytes = Vec::with_capacity(samples.len() * 8);
            for v in samples {
                sample_bytes.extend_from_slice(&v.to_le_bytes());
            }
            let mut children = Vec::new();
            if let Some(s) = scale {
                children.push(FElem::ScalarLinked {
                    tag: SERIES_SCALE_TAG,
                    physical_type: PT_REAL_8,
                    payload: s.to_le_bytes().to_vec(),
                });
            }
            if let Some(o) = offset {
                children.push(FElem::ScalarLinked {
                    tag: SERIES_OFFSET_TAG,
                    physical_type: PT_REAL_8,
                    payload: o.to_le_bytes().to_vec(),
                });
            }
            children.push(FElem::Vector {
                tag: SERIES_VALUES_TAG,
                physical_type: PT_REAL_8,
                count: samples.len() as u32,
                raw: sample_bytes,
            });
            FElem::Collection {
                tag: ONE_SERIES_INSTANCE_TAG,
                children,
            }
        }
        SeriesSpec::Compact {
            start,
            count,
            increment,
        } => {
            let mut vec_bytes = Vec::with_capacity(24);
            vec_bytes.extend_from_slice(&start.to_le_bytes());
            vec_bytes.extend_from_slice(&(*count as f64).to_le_bytes());
            vec_bytes.extend_from_slice(&increment.to_le_bytes());
            FElem::Collection {
                tag: ONE_SERIES_INSTANCE_TAG,
                children: vec![
                    FElem::ScalarEmbedded {
                        tag: STORAGE_METHOD_ID_TAG,
                        physical_type: PT_UNSIGNED_INTEGER_4,
                        embedded: STORAGE_METHOD_INCREMENT.to_le_bytes(),
                    },
                    FElem::Vector {
                        tag: SERIES_VALUES_TAG,
                        physical_type: PT_REAL_8,
                        count: 3,
                        raw: vec_bytes,
                    },
                ],
            }
        }
    }
}

fn vector_char1(tag: Uuid, bytes: &[u8]) -> FElem {
    FElem::Vector {
        tag,
        physical_type: PT_CHAR_1,
        count: bytes.len() as u32,
        raw: bytes.to_vec(),
    }
}

fn vector_char2(tag: Uuid, s: &str) -> FElem {
    // UTF-16LE encode, then null-terminate with a single u16 0.
    let mut units: Vec<u16> = s.encode_utf16().collect();
    units.push(0);
    let mut raw = Vec::with_capacity(units.len() * 2);
    for u in &units {
        raw.extend_from_slice(&u.to_le_bytes());
    }
    // Vector count is number of *elements* (u16 units), not bytes.
    FElem::Vector {
        tag,
        physical_type: PT_CHAR_2,
        count: units.len() as u32,
        raw,
    }
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
            h[18] = 1; // isEmbedded — GSF writer's marker.
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
            out[coll_start..coll_start + 4].copy_from_slice(&(children.len() as i32).to_le_bytes());
            for (i, child) in children.iter().enumerate() {
                let child_header = coll_start + 4 + i * 28;
                write_element(out, child_header, child);
            }
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

/// RFC 1950 zlib-encode `data`. Used only to compress record bodies for
/// the compression fixture.
fn zlib_compress(data: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}
