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

use super::*;
use minifi_native::{MockLogger, MockProcessContext, ProcessorDefinition};
use pqdif::test_fixtures::synthetic;
use serde_json::Value;

/// What a transform produced, detached from the input stream it borrowed.
///
/// `TransformedFlowFile` is tied to the lifetime of the `&mut dyn InputStream`
/// it was built from, so a helper that owns the stream cannot hand one back.
/// Copying out the three things the tests look at is simpler than threading the
/// stream through every call site.
#[derive(Debug)]
struct Outcome {
    relationship: String,
    content: Option<Vec<u8>>,
    attributes: Vec<(String, String)>,
}

impl Outcome {
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .rfind(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Schedule the processor with `properties` layered over the defaults, then run
/// `bytes` through it.
fn run(bytes: &[u8], properties: &[(&str, &str)]) -> Result<Outcome, ProcessError> {
    let mut context = MockProcessContext::new();
    context.properties.extend(properties.iter().copied());
    let logger = MockLogger::new();
    let processor = ParsePqdif::schedule(&context, &logger).expect("should schedule");

    let mut input_stream = Cursor::new(bytes);
    let transformed = processor.transform(&context, &mut input_stream, &logger)?;
    let relationship = transformed.target_relationship().to_owned();
    let attributes = transformed
        .attributes_to_add()
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    Ok(Outcome {
        relationship,
        content: transformed.into_bytes().expect("content readable"),
        attributes,
    })
}

/// Run a fixture and return its body as a UTF-8 string, asserting success.
fn body_of(bytes: &[u8], properties: &[(&str, &str)]) -> String {
    let outcome = run(bytes, properties).expect("should transform");
    assert_eq!(outcome.relationship, SUCCESS.name);
    let content = outcome.content.expect(
        "ParsePqdif always replaces the content on success — routing without \
         changes would leave the FlowFile holding PQDIF binary",
    );
    String::from_utf8(content).expect("output is UTF-8 JSON")
}

fn attribute_of(bytes: &[u8], properties: &[(&str, &str)], name: &str) -> Option<String> {
    run(bytes, properties)
        .expect("should transform")
        .attribute(name)
        .map(str::to_owned)
}

#[test]
fn schedule_succeeds_with_default_values() {
    let processor = ParsePqdif::schedule(&MockProcessContext::new(), &MockLogger::new())
        .expect("defaults should schedule");

    // The defaults are a contract with flow authors, so they are asserted rather
    // than merely exercised.
    assert_eq!(processor.output_format, OutputFormat::Ndjson);
    assert_eq!(processor.checksum_mode, ChecksumMode::WhenPresent);
    assert_eq!(processor.sample_values, SampleValues::Include);
}

#[test]
fn ndjson_writes_one_object_per_line() {
    let (bytes, expected) = synthetic::build_multi_observation(3);
    let body = body_of(&bytes, &[]);

    // The trailing newline is deliberate: a line-oriented reader that treats the
    // last line as terminated rather than delimited gets the same three records.
    assert!(body.ends_with('\n'));
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 3);

    for (index, line) in lines.iter().enumerate() {
        let value: Value = serde_json::from_str(line).expect("each line is a standalone object");
        assert_eq!(value["observation_index"], serde_json::json!(index));
        assert_eq!(
            value["channels"][0]["series"][0]["values"],
            serde_json::json!(expected[index])
        );
        assert!(
            !line.contains('\n'),
            "an embedded newline would break SplitText downstream"
        );
    }
}

#[test]
fn array_writes_a_single_json_array() {
    let (bytes, expected) = synthetic::build_multi_observation(3);
    let body = body_of(&bytes, &[("Output Format", "array")]);

    let value: Value = serde_json::from_str(&body).expect("body is one JSON document");
    let items = value.as_array().expect("top level is an array");
    assert_eq!(items.len(), 3);
    for (index, item) in items.iter().enumerate() {
        assert_eq!(item["observation_index"], serde_json::json!(index));
        assert_eq!(
            item["channels"][0]["series"][0]["values"],
            serde_json::json!(expected[index])
        );
    }
}

#[test]
fn both_formats_carry_structurally_identical_objects() {
    // This is what makes the format property a pure framing choice: a consumer's
    // field paths must not depend on which one was selected.
    let (bytes, _expected) = synthetic::build_multi_observation(3);

    let ndjson: Vec<Value> = body_of(&bytes, &[])
        .lines()
        .map(|line| serde_json::from_str(line).expect("ndjson line parses"))
        .collect();
    let array: Value = serde_json::from_str(&body_of(&bytes, &[("Output Format", "array")]))
        .expect("array body parses");

    assert_eq!(Value::Array(ndjson), array);
}

#[test]
fn an_observation_free_file_yields_a_valid_empty_document() {
    // A container-and-data-source-only file is legal PQDIF. Emitting nothing at
    // all would hand downstream JSON processors an empty FlowFile to choke on,
    // so each format emits its own well-formed empty document instead.
    let bytes = synthetic::build_container_and_data_source_only();

    assert_eq!(body_of(&bytes, &[]), "");
    assert_eq!(body_of(&bytes, &[("Output Format", "array")]), "[]");
    assert_eq!(
        attribute_of(&bytes, &[], OBSERVATION_COUNT_ATTRIBUTE).as_deref(),
        Some("0")
    );
}

#[test]
fn container_metadata_lands_on_attributes() {
    let (bytes, _samples) = synthetic::build();
    let outcome = run(&bytes, &[]).expect("should transform");

    assert_eq!(outcome.attribute("pqdif.writer.version"), Some("1"));
    assert_eq!(outcome.attribute("pqdif.application"), Some("pqdif-rs"));
    assert!(outcome.attribute("pqdif.creation.time").is_some());
}

#[test]
fn format_is_reflected_in_mime_type_and_format_attributes() {
    let (bytes, _samples) = synthetic::build();

    assert_eq!(
        attribute_of(&bytes, &[], MIME_TYPE_ATTRIBUTE).as_deref(),
        Some("application/x-ndjson")
    );
    assert_eq!(
        attribute_of(&bytes, &[], FORMAT_ATTRIBUTE).as_deref(),
        Some("ndjson")
    );

    let array = &[("Output Format", "array")][..];
    assert_eq!(
        attribute_of(&bytes, array, MIME_TYPE_ATTRIBUTE).as_deref(),
        Some("application/json")
    );
    assert_eq!(
        attribute_of(&bytes, array, FORMAT_ATTRIBUTE).as_deref(),
        Some("array")
    );
}

#[test]
fn observation_count_attribute_matches_the_output() {
    let (bytes, _expected) = synthetic::build_multi_observation(4);
    assert_eq!(
        attribute_of(&bytes, &[], OBSERVATION_COUNT_ATTRIBUTE).as_deref(),
        Some("4")
    );
    assert_eq!(body_of(&bytes, &[]).lines().count(), 4);
}

#[test]
fn excluding_sample_values_drops_them_from_every_series() {
    let (bytes, samples) = synthetic::build();
    let body = body_of(&bytes, &[("Include Sample Values", "false")]);
    let value: Value = serde_json::from_str(body.trim_end()).expect("parses");
    let series = &value["channels"][0]["series"][0];

    assert!(series.get("values").is_none());
    assert_eq!(series["sample_count"], serde_json::json!(samples.len()));
}

#[test]
fn non_pqdif_content_routes_to_failure() {
    let result = run(b"this is not a PQDIF file at all, not even close", &[]);
    match result {
        Err(ProcessError::Route(route)) => assert_eq!(route.relationship, FAILURE.name),
        other => panic!("expected a route to failure, got {other:?}"),
    }
}

#[test]
fn truncated_content_routes_to_failure_rather_than_emitting_a_partial_document() {
    // Truncation after the container is the interesting case: the file opens
    // cleanly and only fails partway through the record chain. Emitting the
    // observations read so far would silently hand downstream a subset of the
    // recording, so the whole FlowFile goes to failure instead.
    let (bytes, _expected) = synthetic::build_multi_observation(3);
    let truncated = &bytes[..bytes.len() - 40];

    match run(truncated, &[]) {
        Err(ProcessError::Route(route)) => assert_eq!(route.relationship, FAILURE.name),
        other => panic!("expected a route to failure, got {other:?}"),
    }
}

#[test]
fn a_record_chain_cycle_routes_to_failure() {
    let bytes = synthetic::build_with_cycle();
    match run(&bytes, &[]) {
        Err(ProcessError::Route(route)) => assert_eq!(route.relationship, FAILURE.name),
        other => panic!("expected a route to failure, got {other:?}"),
    }
}

#[test]
fn checksum_verification_property_reaches_the_parser() {
    // The fixture writes correct checksums, so the discriminating input is a
    // file whose checksums have been zeroed the way many real writers leave
    // them: `always` must reject it and the other two modes must accept it.
    let (mut bytes, _samples) = synthetic::build();
    zero_all_checksums(&mut bytes);

    match run(&bytes, &[("Checksum Verification", "always")]) {
        Err(ProcessError::Route(route)) => assert_eq!(route.relationship, FAILURE.name),
        other => panic!("'always' should reject a zeroed checksum, got {other:?}"),
    }

    for mode in ["whenPresent", "never"] {
        let body = body_of(&bytes, &[("Checksum Verification", mode)]);
        assert_eq!(
            body.lines().count(),
            1,
            "'{mode}' should accept a file with zeroed checksums"
        );
    }
}

#[test]
fn a_corrupted_checksum_is_rejected_by_whenpresent() {
    // The complement of the test above: `whenPresent` is a concession to writers
    // that omit the field, not a blanket disabling of verification.
    let (mut bytes, _samples) = synthetic::build();
    corrupt_first_checksum(&mut bytes);

    match run(&bytes, &[("Checksum Verification", "whenPresent")]) {
        Err(ProcessError::Route(route)) => assert_eq!(route.relationship, FAILURE.name),
        other => panic!("'whenPresent' must still verify a present checksum, got {other:?}"),
    }
    // ...and `never` still lets it through, which is the whole point of having it.
    assert_eq!(
        body_of(&bytes, &[("Checksum Verification", "never")])
            .lines()
            .count(),
        1
    );
}

/// Walk the record chain and hand each 64-byte header to `patch`.
///
/// Records are laid out contiguously by the fixture writer, and every header
/// carries `header_size` at 32..36 and `body_size` at 36..40, so this needs no
/// knowledge of which fixture produced the bytes.
fn for_each_record_header(bytes: &mut [u8], mut patch: impl FnMut(&mut [u8])) {
    let mut offset = 0usize;
    while offset + 64 <= bytes.len() {
        let header_size =
            u32::from_le_bytes(bytes[offset + 32..offset + 36].try_into().unwrap()) as usize;
        let body_size =
            u32::from_le_bytes(bytes[offset + 36..offset + 40].try_into().unwrap()) as usize;
        patch(&mut bytes[offset..offset + 64]);
        offset += header_size + body_size;
    }
}

/// The checksum field is header bytes 44..48. Zeroing it imitates a writer that
/// never computed one.
fn zero_all_checksums(bytes: &mut [u8]) {
    for_each_record_header(bytes, |header| header[44..48].fill(0));
}

/// Leave a non-zero but wrong checksum on the first record.
fn corrupt_first_checksum(bytes: &mut [u8]) {
    let mut done = false;
    for_each_record_header(bytes, |header| {
        if !done {
            header[44..48].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
            done = true;
        }
    });
    assert!(done, "fixture should have at least one record");
}

#[test]
fn every_container_attribute_is_declared_in_the_manifest() {
    // The manifest is what PROCESSORS.md and the C2 agent manifest are generated
    // from, so an attribute the processor writes but does not declare is
    // invisible to anyone reading the docs. Driving this from the same function
    // the processor calls means a new container field cannot be added without
    // either declaring it or failing here.
    let bytes = synthetic::build_with_all_container_metadata();
    let (_parser, container) =
        pqdif::LogicalParser::open(Cursor::new(&bytes)).expect("fixture opens");

    let declared: Vec<&str> = ParsePqdif::OUTPUT_ATTRIBUTES
        .iter()
        .map(|attribute| attribute.name)
        .collect();

    let produced = crate::json::container_attributes(&container);
    assert!(
        produced.len() >= 9,
        "the fixture is supposed to populate every container field; got {produced:?}"
    );
    for (name, _value) in &produced {
        assert!(
            declared.contains(&name.as_str()),
            "'{name}' is written but not declared in OUTPUT_ATTRIBUTES"
        );
    }

    for name in [
        MIME_TYPE_ATTRIBUTE,
        OBSERVATION_COUNT_ATTRIBUTE,
        FORMAT_ATTRIBUTE,
    ] {
        assert!(
            declared.contains(&name),
            "'{name}' is written but not declared in OUTPUT_ATTRIBUTES"
        );
    }
}

#[test]
fn relationships_and_properties_are_declared() {
    let relationships: Vec<&str> = ParsePqdif::RELATIONSHIPS
        .iter()
        .map(|relationship| relationship.name)
        .collect();
    assert_eq!(relationships, vec![SUCCESS.name, FAILURE.name]);
    assert_eq!(ParsePqdif::PROPERTIES.len(), 3);
}
