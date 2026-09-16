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

use pqdif::{LogicalParser, PqdifError};

mod fixtures;

#[test]
fn open_yields_container_and_first_observation() {
    let (bytes, samples) = fixtures::synthetic::build();
    let (mut parser, container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");

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
    assert!(
        parser
            .next_observation()
            .expect("terminator ok")
            .is_none(),
    );
}

#[test]
fn scale_and_offset_are_applied_to_values() {
    let (bytes, expected) = fixtures::synthetic::build_with_scale_offset();
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let observation = parser
        .next_observation()
        .expect("ok")
        .expect("present");
    let series = &observation.channels[0].series[0];
    assert_eq!(series.scale, 10.0);
    assert_eq!(series.offset, 100.0);
    assert_eq!(series.values, expected);
}

#[test]
fn compact_series_expands_to_progression() {
    let (bytes, expected) = fixtures::synthetic::build_with_compact_series();
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let observation = parser
        .next_observation()
        .expect("ok")
        .expect("present");
    let series = &observation.channels[0].series[0];
    assert_eq!(series.values, expected);
}

#[test]
fn multi_observation_chain_is_walked_in_order() {
    let (bytes, expected) = fixtures::synthetic::build_multi_observation(3);
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");

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
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");

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
    let (mut parser, container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");
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
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");

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
    let (_parser, container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");
    assert_eq!(container.application.as_deref(), Some(expected.as_str()));
}

#[test]
fn zlib_compressed_bodies_round_trip() {
    let (bytes, samples) = fixtures::synthetic::build_zlib_compressed();
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let obs = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs.channels[0].series[0].values, samples);
}

#[test]
fn two_channels_are_decoded_independently() {
    let (bytes, [s0, s1]) = fixtures::synthetic::build_two_channels();
    let (mut parser, _container) =
        LogicalParser::open(Cursor::new(&bytes)).expect("open");
    let obs = parser.next_observation().expect("ok").expect("present");
    assert_eq!(obs.channels.len(), 2);
    assert_eq!(obs.channels[0].name.as_deref(), Some("CH_LEFT"));
    assert_eq!(obs.channels[0].series[0].values, s0);
    assert_eq!(obs.channels[1].name.as_deref(), Some("CH_RIGHT"));
    assert_eq!(obs.channels[1].series[0].values, s1);
}
