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
//! Usage: `cargo run -p pqdif --example dump_pqdif -- [--checksums=MODE] <path>`
//!
//! Prints the Container record fields, then one summary line per
//! Observation record. Useful as a smoke test against real files —
//! synthetic fixtures live in the crate's `tests/`.
//!
//! `--checksums=MODE` selects the Adler-32 verification policy:
//!
//! - `always` (default) — every record must carry a matching checksum.
//! - `when-present` — verify records with a non-zero checksum, accept those
//!   that leave the field `0x00000000`. The right choice for real files from
//!   a trusted source, since writers vary in whether they populate it: the
//!   reference GSF implementation computes checksums on read but does not
//!   require them to match, and several third-party writers omit them.
//! - `never` — skip verification entirely.
//!
//! `--no-verify-checksums` is kept as an alias for `--checksums=never`.

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use pqdif::{ChecksumMode, LogicalParser, OpenOptions};

const USAGE: &str = "usage: dump_pqdif [--checksums=always|when-present|never] <path-to-.pqd>";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut checksum_mode = ChecksumMode::default();
    let mut path: Option<String> = None;
    for arg in args.by_ref() {
        match arg.as_str() {
            "--no-verify-checksums" => checksum_mode = ChecksumMode::Never,
            flag if flag.starts_with("--checksums=") => {
                checksum_mode = match flag.trim_start_matches("--checksums=") {
                    "always" => ChecksumMode::Always,
                    "when-present" => ChecksumMode::WhenPresent,
                    "never" => ChecksumMode::Never,
                    other => {
                        eprintln!("unknown checksum mode {other:?}\n{USAGE}");
                        return ExitCode::from(2);
                    }
                };
            }
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with("--") => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
            _ => {
                path = Some(arg);
                break;
            }
        }
    }
    let Some(path) = path else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open {path}: {e}");
            return ExitCode::from(1);
        }
    };

    let options = OpenOptions { checksum_mode };
    let (mut parser, container) =
        match LogicalParser::open_with_options(BufReader::new(file), options) {
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
