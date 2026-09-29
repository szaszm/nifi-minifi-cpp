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

//! Integration tests: open the family of synthetic fixtures through the
//! logical parser and check the typed fields the reader exposes to callers.

use std::io::Cursor;

use pqdif::test_fixtures as fixtures;
use pqdif::{ChecksumMode, LogicalParser, OpenOptions, PqdifError};

/// Walk the record chain in `bytes` and hand each record's header offset to
/// `patch`. Records are laid out contiguously by the fixture writer, and each
/// header carries `header_size` at 32..36 and `body_size` at 36..40, so the
/// walk needs no knowledge of which fixture produced the file.
fn for_each_record_header(bytes: &mut [u8], mut patch: impl FnMut(&mut [u8])) -> usize {
    let mut offset = 0usize;
    let mut count = 0usize;
    while offset + 64 <= bytes.len() {
        let header_size =
            u32::from_le_bytes(bytes[offset + 32..offset + 36].try_into().unwrap()) as usize;
        let body_size =
            u32::from_le_bytes(bytes[offset + 36..offset + 40].try_into().unwrap()) as usize;
        patch(&mut bytes[offset..offset + 64]);
        count += 1;
        offset += header_size + body_size;
    }
    count
}

/// Zero every record's checksum field (header bytes 44..48), imitating a
/// writer that never computed one. Returns the number of records patched.
fn strip_all_checksums(bytes: &mut [u8]) -> usize {
    for_each_record_header(bytes, |header| header[44..48].fill(0))
}

#[test]
fn open_yields_container_and_first_observation() {
    let (bytes, samples) = fixtures::synthetic::build();
    let (mut parser, container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");

    // Container-level fields we hand-encoded in the fixture.
    assert_eq!(container.writer_version, 1);
    assert_eq!(container.application.as_deref(), Some("pqdif-rs"));
    assert!(container.creation_time.is_some());

    let observation = parser
        .next_observation()
        .expect("observation decoded")
        .expect("observation present");
    assert_eq!(observation.name.as_deref(), Some("OBS1"));
    assert!(observation.time_start.is_some());
    assert_eq!(observation.channels.len(), 1);

    let channel = &observation.channels[0];
    assert_eq!(channel.channel_definition_index, 0);
    // The DataSource in the fixture named its single channel "CH1".
    assert_eq!(channel.name.as_deref(), Some("CH1"));
    assert_eq!(channel.series.len(), 1);
    assert_eq!(channel.series[0].values, samples);
    // Defaults when no scale/offset scalars are present.
    assert_eq!(channel.series[0].scale, 1.0);
    assert_eq!(channel.series[0].offset, 0.0);

    // Chain terminates.
    assert!(parser.next_observation().expect("terminator ok").is_none(),);
}

#[test]
fn scale_and_offset_are_applied_to_values() {
    let (bytes, expected) = fixtures::synthetic::build_with_scale_offset();
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let observation = parser.next_observation().expect("ok").expect("present");
    let series = &observation.channels[0].series[0];
    assert_eq!(series.scale, 10.0);
    assert_eq!(series.offset, 100.0);
    assert_eq!(series.values, expected);
}

#[test]
fn compact_series_expands_to_progression() {
    let (bytes, expected) = fixtures::synthetic::build_with_compact_series();
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let observation = parser.next_observation().expect("ok").expect("present");
    let series = &observation.channels[0].series[0];
    assert_eq!(series.values, expected);
}

#[test]
fn multi_observation_chain_is_walked_in_order() {
    let (bytes, expected) = fixtures::synthetic::build_multi_observation(3);
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");

    let mut names = Vec::new();
    let mut values = Vec::new();
    while let Some(obs) = parser.next_observation().expect("ok") {
        names.push(obs.name.clone().unwrap_or_default());
        values.push(obs.channels[0].series[0].values.clone());
    }
    assert_eq!(names, vec!["OBS1", "OBS2", "OBS3"]);
    assert_eq!(values, expected);
}

#[test]
fn blank_record_is_skipped_between_observations() {
    let (bytes, [s0, s1]) = fixtures::synthetic::build_with_blank_record();
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");

    let obs0 = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs0.name.as_deref(), Some("OBS_A"));
    assert_eq!(obs0.channels[0].series[0].values, s0);

    // The Blank record between the two observations is skipped silently.
    let obs1 = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs1.name.as_deref(), Some("OBS_B"));
    assert_eq!(obs1.channels[0].series[0].values, s1);

    assert!(parser.next_observation().expect("ok").is_none());
}

#[test]
fn second_container_is_ignored() {
    let (bytes, samples) = fixtures::synthetic::build_with_second_container();
    let (mut parser, container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");
    assert_eq!(container.writer_version, 1);

    // The stray second Container record is silently skipped; iteration still
    // reaches the Observation past it.
    let obs = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs.name.as_deref(), Some("OBS1"));
    assert_eq!(obs.channels[0].series[0].values, samples);
}

#[test]
fn data_source_swap_affects_channel_names() {
    let (bytes, [s0, s1]) = fixtures::synthetic::build_data_source_swap();
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");

    let obs0 = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs0.channels[0].name.as_deref(), Some("CH_A"));
    assert_eq!(obs0.channels[0].series[0].values, s0);

    let obs1 = parser.next_observation().expect("ok").expect("present");
    // After the second DataSource, channel index 0 resolves to a new name.
    assert_eq!(obs1.channels[0].name.as_deref(), Some("CH_B"));
    assert_eq!(obs1.channels[0].series[0].values, s1);
}

#[test]
fn non_container_first_record_is_rejected() {
    let bytes = fixtures::synthetic::build_non_container_first();
    match LogicalParser::open(Cursor::new(&bytes)) {
        Err(PqdifError::BadHeader(_)) => {}
        Err(e) => panic!("expected BadHeader, got {e:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn utf16_application_string_decodes() {
    let (bytes, expected) = fixtures::synthetic::build_with_utf16_application();
    let (_parser, container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");
    assert_eq!(container.application.as_deref(), Some(expected.as_str()));
}

#[test]
fn zlib_compressed_bodies_round_trip() {
    let (bytes, samples) = fixtures::synthetic::build_zlib_compressed();
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let obs = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs.channels[0].series[0].values, samples);
}

#[test]
fn two_channels_are_decoded_independently() {
    let (bytes, [s0, s1]) = fixtures::synthetic::build_two_channels();
    let (mut parser, _container) = LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let obs = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs.channels.len(), 2);
    assert_eq!(obs.channels[0].name.as_deref(), Some("CH_LEFT"));
    assert_eq!(obs.channels[0].series[0].values, s0);
    assert_eq!(obs.channels[1].name.as_deref(), Some("CH_RIGHT"));
    assert_eq!(obs.channels[1].series[0].values, s1);
}

#[test]
fn checksumless_file_is_rejected_by_default() {
    // The strict default must fire on the *Container*, i.e. inside `open`,
    // not merely on later records.
    let (mut bytes, _) = fixtures::synthetic::build_multi_observation(3);
    assert_eq!(strip_all_checksums(&mut bytes), 5, "container + ds + 3 obs");
    assert!(
        matches!(
            LogicalParser::open(Cursor::new(&bytes)),
            Err(PqdifError::ChecksumMismatch { expected: 0, .. })
        ),
        "a file with no checksums must fail the default strict mode",
    );
}

#[test]
fn when_present_walks_a_checksumless_file_end_to_end() {
    // The motivating case: a real file from a trusted writer that populates
    // no checksums at all. Every record must decode, including the Container
    // (which is read during `open`, before any post-hoc setter could apply).
    let (mut bytes, expected) = fixtures::synthetic::build_multi_observation(3);
    strip_all_checksums(&mut bytes);

    let (mut parser, container) = LogicalParser::open_with_options(
        Cursor::new(&bytes),
        OpenOptions {
            checksum_mode: ChecksumMode::WhenPresent,
        },
    )
    .expect("WhenPresent must accept a fully checksumless file");
    assert_eq!(container.application.as_deref(), Some("pqdif-rs"));

    let mut values = Vec::new();
    while let Some(obs) = parser.next_observation().expect("observation ok") {
        values.push(obs.channels[0].series[0].values.clone());
    }
    assert_eq!(values, expected);
}

#[test]
fn when_present_still_rejects_a_corrupt_body() {
    // Leniency is scoped to absent checksums. The fixture writer computes
    // real ones, so corrupting a body must still be caught under
    // WhenPresent — otherwise the mode would be indistinguishable from
    // disabling verification.
    let (mut bytes, _) = fixtures::synthetic::build();
    let container_body_start = 64;
    bytes[container_body_start] ^= 0xff;

    assert!(
        matches!(
            LogicalParser::open_with_options(
                Cursor::new(&bytes),
                OpenOptions {
                    checksum_mode: ChecksumMode::WhenPresent,
                },
            ),
            Err(PqdifError::ChecksumMismatch { .. })
        ),
        "WhenPresent must verify records that do carry a checksum",
    );
}

#[test]
fn when_present_is_a_mix_per_record_not_per_file() {
    // Strip the checksum from the Container only, leaving the DataSource and
    // Observation checksums intact and correct. A per-file "does this writer
    // use checksums?" heuristic would get this wrong; the decision is made
    // independently for each record.
    let (mut bytes, samples) = fixtures::synthetic::build();
    bytes[44..48].fill(0);

    let (mut parser, _container) = LogicalParser::open_with_options(
        Cursor::new(&bytes),
        OpenOptions {
            checksum_mode: ChecksumMode::WhenPresent,
        },
    )
    .expect("container with a stripped checksum is accepted");
    let obs = parser
        .next_observation()
        .expect("later records still verify and pass")
        .expect("present");
    assert_eq!(obs.channels[0].series[0].values, samples);
}
