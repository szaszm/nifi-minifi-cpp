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

//! Minimal `.pqd` dumper.
//!
//! Usage: `cargo run -p pqdif --example dump_pqdif -- <path>`
//!
//! Prints the Container record fields, then one summary line per
//! Observation record. Useful as a smoke test against real files —
//! synthetic fixtures live in the crate's `tests/`.

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use pqdif::LogicalParser;

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: dump_pqdif <path-to-.pqd>");
        return ExitCode::from(2);
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open {path}: {e}");
            return ExitCode::from(1);
        }
    };

    let (mut parser, container) = match LogicalParser::open(BufReader::new(file)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("open failed: {e}");
            return ExitCode::from(1);
        }
    };

    println!("Container:");
    println!("  writer_version   = {}", container.writer_version);
    println!("  application      = {:?}", container.application);
    println!("  author           = {:?}", container.author);
    println!("  title            = {:?}", container.title);
    println!("  creation_time    = {:?}", container.creation_time);
    println!("  last_saved       = {:?}", container.last_saved);

    let mut n = 0usize;
    loop {
        match parser.next_observation() {
            Ok(Some(obs)) => {
                n += 1;
                let sample_count: usize = obs
                    .channels
                    .iter()
                    .flat_map(|c| c.series.iter())
                    .map(|s| s.values.len())
                    .sum();
                println!(
                    "Observation #{n}: name={:?} time_start={:?} channels={} samples={}",
                    obs.name,
                    obs.time_start,
                    obs.channels.len(),
                    sample_count,
                );
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("read failed at observation #{}: {e}", n + 1);
                return ExitCode::from(1);
            }
        }
    }

    println!("Total observations: {n}");
    ExitCode::SUCCESS
}
