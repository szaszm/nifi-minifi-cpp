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

//! Integration test that runs against a user-supplied real `.pqd` file.
//!
//! Doubly opt-in, because this crate deliberately ships no `.pqd` sample:
//! the file is compiled only under the `real-file-tests` feature, and the
//! test is `#[ignore]`d so it does not run even then unless asked for by
//! name. That keeps both `cargo test` and `cargo test --all-features` green
//! on a bare checkout, and — unlike skipping at runtime — a skip is reported
//! as `ignored` rather than masquerading as a pass.
//!
//! Because running it is an explicit request, an unset `PQDIF_TEST_FILE` is
//! a hard failure rather than a silent no-op.
//!
//! ```text
//! PQDIF_TEST_FILE=/path/to/file.pqd \
//!     cargo test -p pqdif --features real-file-tests --test real_file_smoke \
//!     -- --ignored --nocapture
//! ```
//!
//! Two independently-authored samples the maintainers used to validate this
//! test:
//!
//! - `PowerPeg-PQDIF-Read-Write/PQDIF-Application/consoleUI/bin/Debug/net6.0/TestCase/t1.pqd`
//!   from <https://github.com/pangchunhei/PowerPeg-PQDIF-Read-Write>
//!   (121 observations, ~2.9 MiB).
//! - Any small sample produced by the GSF PQDIF library or a PQView writer.
//!
//! Real-world writers usually leave the Adler-32 checksum field zero
//! (matching the GSF reader, which computes but does not require it), so the
//! test opens the file with [`ChecksumMode::WhenPresent`] rather than
//! disabling verification outright — whatever the supplied file happens to
//! populate still gets checked.

#![cfg(feature = "real-file-tests")]

use std::fs::File;
use std::io::BufReader;

use pqdif::{ChecksumMode, LogicalParser, OpenOptions};

#[test]
#[ignore = "needs a real .pqd file; set PQDIF_TEST_FILE and pass --ignored"]
fn walks_a_real_pqdif_file() {
    // Reaching this point means someone passed --ignored, i.e. asked for the
    // test by name — so a missing path is a mistake to report, not a
    // condition to skip past.
    let path = std::env::var("PQDIF_TEST_FILE").expect(
        "PQDIF_TEST_FILE must name a real .pqd file to run this test \
         (see the module doc comment)",
    );

    let file = File::open(&path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let (mut parser, container) = LogicalParser::open_with_options(
        BufReader::new(file),
        OpenOptions {
            checksum_mode: ChecksumMode::WhenPresent,
        },
    )
    .expect("logical open");

    eprintln!(
        "container: application={:?} writer_version={} creation_time={:?}",
        container.application, container.writer_version, container.creation_time,
    );

    let mut n = 0usize;
    let mut total_samples = 0usize;
    while let Some(obs) = parser.next_observation().expect("observation decode") {
        n += 1;
        for channel in &obs.channels {
            for series in &channel.series {
                total_samples += series.values.len();
            }
        }
    }
    eprintln!(
        "decoded {n} observation(s), {total_samples} sample(s) total"
    );
    assert!(
        n >= 1,
        "expected at least one Observation record in {path}, got {n}"
    );
}
