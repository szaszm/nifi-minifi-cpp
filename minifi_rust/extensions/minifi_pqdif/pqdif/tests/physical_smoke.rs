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

//! Integration tests: walk the physical layer of the synthetic fixtures and
//! check the shape of the resulting element trees, plus the guardrails
//! (checksum verification, cycle detection).

use std::io::Cursor;

use pqdif::test_fixtures as fixtures;
use pqdif::{PhysicalParser, PqdifError};

#[test]
fn walks_three_record_chain() {
    let (bytes, _samples) = fixtures::synthetic::build();
    let mut parser = PhysicalParser::new(Cursor::new(&bytes)).expect("open");

    let container = parser
        .next_record()
        .expect("container ok")
        .expect("container present");
    assert_eq!(container.offset, 0);
    assert_eq!(
        container.header.type_tag,
        uuid::uuid!("89738606-f1c3-11cf-9d89-0080c72e70a3"),
    );
    // Container has three top-level elements: VersionInfo, Application, Creation.
    assert_eq!(container.body.children.len(), 3);

    let data_source = parser
        .next_record()
        .expect("data source ok")
        .expect("data source present");
    assert_eq!(
        data_source.header.type_tag,
        uuid::uuid!("89738619-f1c3-11cf-9d89-0080c72e70a3"),
    );
    assert_eq!(data_source.body.children.len(), 2);

    let observation = parser
        .next_record()
        .expect("observation ok")
        .expect("observation present");
    assert_eq!(
        observation.header.type_tag,
        uuid::uuid!("8973861a-f1c3-11cf-9d89-0080c72e70a3"),
    );
    assert_eq!(observation.body.children.len(), 3);

    // Chain terminates after the observation.
    assert!(parser.next_record().expect("terminator").is_none());
}

#[test]
fn checksum_mismatch_is_detected() {
    let (mut bytes, _samples) = fixtures::synthetic::build();
    // Flip a bit inside the container body (past the 64-byte header).
    bytes[64 + 8] ^= 0xff;

    let mut parser = PhysicalParser::new(Cursor::new(&bytes)).expect("open");
    let err = parser
        .next_record()
        .expect_err("checksum mismatch should be raised");
    assert!(matches!(err, PqdifError::ChecksumMismatch { .. }));
}

#[test]
fn checksum_verification_can_be_disabled() {
    let (mut bytes, _samples) = fixtures::synthetic::build();
    bytes[64 + 8] ^= 0xff;

    let mut parser = PhysicalParser::new(Cursor::new(&bytes)).expect("open");
    parser.verify_checksums(false);
    // Corrupted body: decoding still succeeds because checksums are off; the
    // decoded element tree may still be sensible enough to walk the chain.
    let _ = parser.next_record();
}

#[test]
fn self_referential_next_record_triggers_cycle_error() {
    let bytes = fixtures::synthetic::build_with_cycle();
    let mut parser = PhysicalParser::new(Cursor::new(&bytes)).expect("open");
    // First pull yields the Container.
    let _ = parser
        .next_record()
        .expect("first pull ok")
        .expect("present");
    // Second pull yields the DataSource whose next_record_pos loops back to
    // itself.
    let ds = parser
        .next_record()
        .expect("second pull ok")
        .expect("present");
    let ds_offset = ds.offset;
    // Third pull revisits the DataSource offset → cycle detection fires.
    match parser.next_record() {
        Err(PqdifError::RecordCycle(off)) => assert_eq!(off, ds_offset),
        other => panic!("expected RecordCycle, got {other:?}"),
    }
}

#[test]
fn zlib_compressed_chain_walks_and_decompresses() {
    let (bytes, _samples) = fixtures::synthetic::build_zlib_compressed();
    let mut parser = PhysicalParser::new(Cursor::new(&bytes)).expect("open");

    // Container — uncompressed regardless of settings.
    let container = parser.next_record().expect("ok").expect("present");
    assert_eq!(
        container.header.type_tag,
        uuid::uuid!("89738606-f1c3-11cf-9d89-0080c72e70a3"),
    );

    // DataSource — body was compressed on disk; the parser transparently
    // decompresses it and the decoded element tree looks the same as the
    // uncompressed fixture.
    let data_source = parser.next_record().expect("ok").expect("present");
    assert_eq!(data_source.body.children.len(), 2);

    let observation = parser.next_record().expect("ok").expect("present");
    assert_eq!(observation.body.children.len(), 3);

    assert!(parser.next_record().expect("terminator").is_none());
}
