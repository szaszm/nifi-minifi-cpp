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

//! Integration test: open the synthetic fixture through the logical parser
//! and check the typed fields the reader exposes to callers.

use std::io::Cursor;

use pqdif::LogicalParser;

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

    // Chain terminates.
    assert!(
        parser
            .next_observation()
            .expect("terminator ok")
            .is_none(),
    );
}
