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

//! Tests for the JSON projection.
//!
//! Every fixture here is driven through the real parser rather than by
//! hand-constructing `ObservationRecord` values: the point of these tests is
//! that the *contract* holds for what the reader actually produces, and a
//! hand-built record could satisfy the serializer while diverging from anything
//! the parser emits.

use super::*;
use pqdif::LogicalParser;
use pqdif::test_fixtures::synthetic;
use std::io::Cursor;

/// Parse a fixture and return its container plus every observation, already
/// paired with the data source that was in force when it was read.
fn parse(
    bytes: &[u8],
) -> (
    ContainerRecord,
    Vec<(ObservationRecord, Option<DataSourceRecord>)>,
) {
    let (mut parser, container) = LogicalParser::open(Cursor::new(bytes)).expect("fixture opens");
    let mut observations = Vec::new();
    while let Some(observation) = parser.next_observation().expect("fixture parses") {
        observations.push((observation, parser.current_data_source().cloned()));
    }
    (container, observations)
}

/// Serialize the single observation of a single-observation fixture.
fn single_observation(bytes: &[u8], samples: SampleValues) -> Value {
    let (_container, observations) = parse(bytes);
    assert_eq!(observations.len(), 1, "fixture should hold one observation");
    let (observation, data_source) = &observations[0];
    observation_to_json(observation, 0, data_source.as_ref(), samples)
}

#[test]
fn canonical_fixture_round_trips_to_the_documented_shape() {
    let (bytes, samples) = synthetic::build();
    let value = single_observation(&bytes, SampleValues::Include);

    assert_eq!(value["observation_index"], json!(0));
    assert_eq!(value["name"], json!("OBS1"));
    assert_eq!(value["data_source"], json!("synthetic"));
    assert!(
        value["time_start"].is_string(),
        "the fixture encodes tagTimeStart"
    );

    let channels = value["channels"].as_array().expect("channels is an array");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0]["channel_definition_index"], json!(0));
    assert_eq!(channels[0]["name"], json!("CH1"));

    let series = channels[0]["series"]
        .as_array()
        .expect("series is an array");
    assert_eq!(series.len(), 1);
    assert_eq!(series[0]["scale"], json!(1.0));
    assert_eq!(series[0]["offset"], json!(0.0));
    assert_eq!(series[0]["sample_count"], json!(samples.len()));
    assert_eq!(series[0]["values"], json!(samples));
}

#[test]
fn container_metadata_is_absent_from_the_content() {
    // The rule that makes downstream splitting work: container fields live on
    // attributes only, so nothing in the content is lost when the observations
    // are fanned out.
    let (bytes, _samples) = synthetic::build();
    let value = single_observation(&bytes, SampleValues::Include);
    let object = value.as_object().expect("observation is an object");

    for forbidden in ["application", "writer_version", "container", "author"] {
        assert!(
            !object.contains_key(forbidden),
            "container field '{forbidden}' leaked into the observation object"
        );
    }
}

#[test]
fn excluding_samples_keeps_the_count_and_drops_the_values() {
    let (bytes, samples) = synthetic::build();
    let value = single_observation(&bytes, SampleValues::Exclude);
    let series = &value["channels"][0]["series"][0];

    assert_eq!(series["sample_count"], json!(samples.len()));
    assert!(
        series.get("values").is_none(),
        "values must be omitted entirely, not emptied — an empty array would be \
         indistinguishable from a genuinely empty series"
    );
}

#[test]
fn absent_optional_fields_are_omitted_rather_than_null() {
    let (bytes, _samples) = synthetic::build();
    let value = single_observation(&bytes, SampleValues::Include);

    // The canonical fixture sets tagTimeStart but neither tagTimeCreate nor
    // tagTimeTriggered, which is exactly the sparseness real files exhibit.
    assert!(value.get("time_start").is_some());
    assert!(value.get("time_create").is_none());
    assert!(value.get("time_triggered").is_none());

    fn assert_no_nulls(value: &Value, path: &str) {
        match value {
            Value::Null => panic!("null at {path}"),
            Value::Object(map) => {
                for (key, child) in map {
                    assert_no_nulls(child, &format!("{path}.{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    assert_no_nulls(child, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }
    assert_no_nulls(&value, "observation");
}

#[test]
fn enumerated_ids_carry_both_name_and_raw_id() {
    let (bytes, _samples) = synthetic::build();
    let value = single_observation(&bytes, SampleValues::Include);
    let channel = &value["channels"][0];

    // The fixture writes no tagPhaseID / tagQuantityMeasuredID, so the reader
    // falls back to each group's None sentinel. That is still a value the
    // registry resolves, which is what makes it a usable assertion here: name
    // and id are emitted as a pair regardless of which id it happens to be.
    assert_eq!(channel["phase_id"], json!(phase::NONE));
    assert_eq!(channel["phase"], json!("None"));
    assert_eq!(
        channel["quantity_measured_id"],
        json!(quantity_measured::NONE)
    );
    assert_eq!(channel["quantity_measured"], json!("None"));
}

#[test]
fn scale_and_offset_are_reported_even_though_values_are_pre_scaled() {
    let (bytes, expected) = synthetic::build_with_scale_offset();
    let value = single_observation(&bytes, SampleValues::Include);
    let series = &value["channels"][0]["series"][0];

    assert_eq!(series["scale"], json!(10.0));
    assert_eq!(series["offset"], json!(100.0));
    // The values are already scaled; scale/offset are documentation of how the
    // file encoded them, not a transform the consumer still has to apply.
    assert_eq!(series["values"], json!(expected));
}

#[test]
fn every_channel_of_a_multi_channel_observation_is_serialized() {
    let (bytes, [left, right]) = synthetic::build_two_channels();
    let value = single_observation(&bytes, SampleValues::Include);
    let channels = value["channels"].as_array().expect("channels");

    assert_eq!(channels.len(), 2);
    assert_eq!(channels[0]["name"], json!("CH_LEFT"));
    assert_eq!(channels[0]["series"][0]["values"], json!(left));
    assert_eq!(channels[1]["name"], json!("CH_RIGHT"));
    assert_eq!(channels[1]["series"][0]["values"], json!(right));
}

#[test]
fn observation_index_tracks_position_in_the_file() {
    let (bytes, expected) = synthetic::build_multi_observation(3);
    let (_container, observations) = parse(&bytes);
    assert_eq!(observations.len(), 3);

    for (index, (observation, data_source)) in observations.iter().enumerate() {
        let value = observation_to_json(
            observation,
            index,
            data_source.as_ref(),
            SampleValues::Include,
        );
        assert_eq!(value["observation_index"], json!(index));
        assert_eq!(value["name"], json!(format!("OBS{}", index + 1)));
        assert_eq!(
            value["channels"][0]["series"][0]["values"],
            json!(expected[index])
        );
    }
}

#[test]
fn a_data_source_swap_mid_file_re_resolves_channel_names() {
    // The data source in force can change between observations; each one must be
    // rendered against the definition that was current when it was read, not
    // against the first or last one in the file.
    let (bytes, _samples) = synthetic::build_data_source_swap();
    let (_container, observations) = parse(&bytes);
    assert_eq!(observations.len(), 2);

    let names: Vec<String> = observations
        .iter()
        .enumerate()
        .map(|(index, (observation, data_source))| {
            let value = observation_to_json(
                observation,
                index,
                data_source.as_ref(),
                SampleValues::Exclude,
            );
            value["channels"][0]["name"]
                .as_str()
                .expect("channel name resolved from the data source in force")
                .to_owned()
        })
        .collect();

    assert_ne!(
        names[0], names[1],
        "the fixture swaps data sources, so the two observations must not \
         resolve to the same channel name"
    );
}

#[test]
fn container_attributes_are_prefixed_and_carry_what_the_file_declared() {
    let (bytes, _samples) = synthetic::build();
    let (container, _observations) = parse(&bytes);
    let attributes = container_attributes(&container);

    for (name, _value) in &attributes {
        assert!(
            name.starts_with(ATTRIBUTE_PREFIX),
            "'{name}' is not under the {ATTRIBUTE_PREFIX} namespace"
        );
    }

    let lookup = |key: &str| {
        attributes
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(lookup("pqdif.writer.version"), Some("1"));
    assert_eq!(lookup("pqdif.application"), Some("pqdif-rs"));
    assert!(lookup("pqdif.creation.time").is_some());
    // The fixture declares no author or title, and an attribute set to the empty
    // string reads as "the file said it was blank" rather than "absent".
    assert_eq!(lookup("pqdif.author"), None);
    assert_eq!(lookup("pqdif.title"), None);
}

#[test]
fn utf16_container_strings_survive_into_attributes() {
    let (bytes, expected_application) = synthetic::build_with_utf16_application();
    let (container, _observations) = parse(&bytes);
    let attributes = container_attributes(&container);

    let application = attributes
        .iter()
        .find(|(name, _)| name == "pqdif.application")
        .map(|(_, value)| value.as_str());
    assert_eq!(application, Some(expected_application.as_str()));
}

#[test]
fn timestamps_are_iso8601_without_a_zone_suffix() {
    let (bytes, _samples) = synthetic::build();
    let value = single_observation(&bytes, SampleValues::Exclude);
    let time_start = value["time_start"]
        .as_str()
        .expect("time_start is a string");

    // PQDIF stores no offset, so asserting one would be a lie. Parsing it back
    // as naive is the check that we have not appended a 'Z' or an offset.
    NaiveDateTime::parse_from_str(time_start, "%Y-%m-%dT%H:%M:%S%.f")
        .unwrap_or_else(|e| panic!("'{time_start}' is not a naive ISO 8601 timestamp: {e}"));
    assert!(!time_start.ends_with('Z'));
    assert!(!time_start.contains('+'));
}
