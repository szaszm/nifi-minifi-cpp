<!--
  Licensed to the Apache Software Foundation (ASF) under one or more
  contributor license agreements.  See the NOTICE file distributed with
  this work for additional information regarding copyright ownership.
  The ASF licenses this file to You under the Apache License, Version 2.0
  (the "License"); you may not use this file except in compliance with
  the License.  You may obtain a copy of the License at
      http://www.apache.org/licenses/LICENSE-2.0
  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# pqdif

Pure-Rust reader for the **Power Quality Data Interchange Format** (PQDIF,
IEEE Std 1159.3) — the binary interchange format for power-quality
measurement data used by disturbance recorders, PMUs, and PQ monitors.

> **Status: reader-only tech preview.** This crate lands in phases so the
> reader can be exercised against real files before the writer is designed.
> See the roadmap below.

This crate knows nothing about MiNiFi and has no dependency on it, so it stays
publishable on its own. It lives inside the `minifi_pqdif` extension because
that is currently its only consumer; the MiNiFi-facing processor and the JSON
projection live in the parent directory.

## Scope

- **Physical layer** — record header parsing, element tree walk (Scalar /
  Vector / Collection), per-record `Adler-32` verification (default on),
  per-record `zlib`-wrapped deflate decompression, cycle-detection on the
  linked-list `next_record_pos` chain.
- **Logical layer** — typed record objects (`ContainerRecord`,
  `DataSourceRecord`, `ObservationRecord`), channel/series decoding with
  scale + offset applied, `start/count/increment` compact series form,
  silent-skip of `Blank` records (matches the reference implementation).
- **Tag registry** — structural tags plus the four enumerated tag-value
  groups needed to interpret channel data (`QuantityMeasured`,
  `QuantityCharacteristic`, `SeriesValueType`, `Phase`). Additional
  vendor / equipment / instrument-type / disturbance-category enumerations
  land in Phase 3.

## Roadmap

| Phase | Contents                                                                                                     | Status       |
|-------|--------------------------------------------------------------------------------------------------------------|--------------|
| 1     | Reader library (this crate)                                                                                  | in progress  |
| 2     | Writer library (symmetric physical + logical writer)                                                         | planned      |
| 3     | Full tag / value registry, `MonitorSettingsRecord`, IEEE 1159.3-2025 & Corrigendum-1-2024 deltas             | planned      |
| 4     | MiNiFi Rust extension wrapping this crate — `ParsePqdif` in the parent `minifi_pqdif` crate; `WritePqdif` waits on Phase 2 | reader done |

## Example

```rust,ignore
use pqdif::LogicalParser;
use std::fs::File;

let file = File::open("recording.pqd")?;
let (mut parser, container) = LogicalParser::open(file)?;
println!("writer: {}, compatible: {}", container.writer_name, container.compatible_version);

while let Some(obs) = parser.next_observation()? {
    for channel in &obs.channels {
        println!("channel {}: {} samples", channel.name, channel.series.len());
    }
}
# Ok::<(), pqdif::PqdifError>(())
```

## Reference implementation

The binary format was ported from the reference implementation:

- **[Grid Protection Alliance / GSF.PQDIF](https://github.com/GridProtectionAlliance/gsf)** —
  C# reference implementation (MIT licensed). This crate ships a verbatim
  copy of its `TagDefinitions.xml` at `tags/tag_definitions.xml`; see
  `tags/ATTRIBUTION.md` and `tags/LICENSE-GSF-PQDIF`.
- **[gemstone / pqdif](https://github.com/gemstone/pqdif)** — GPA's
  modernized successor package (also MIT). Byte-identical tag definitions.

Only the tag-registry XML is vendored; the parser is a from-scratch idiomatic
Rust port that returns `Result` instead of throwing, uses zero-copy iteration
over borrowed byte slices where possible, and adds a deliberately-stricter
checksum policy. (GSF computes but does not verify the record checksum.)

### Checksum modes

PQDIF has no "checksum absent" flag — writers that do not compute one simply
leave the header field `0x00000000`, which is common in the wild. `ChecksumMode`
therefore offers three policies:

| Mode | Behaviour |
| --- | --- |
| `Always` (default) | Verify every record; a zero field is treated as corruption. Use for untrusted input. |
| `WhenPresent` | Verify records whose checksum is non-zero, accept those that leave it zero. Use for real files from **trusted** sources, where writers vary. |
| `Never` | Skip verification; the field is still exposed on `RecordHeader::checksum`. |

`WhenPresent` decides per record, not per file, so a file that checksums some
records and not others is still fully checked wherever a checksum exists.

```rust
use pqdif::{ChecksumMode, LogicalParser, OpenOptions};

let (mut parser, container) = LogicalParser::open_with_options(
    file,
    OpenOptions { checksum_mode: ChecksumMode::WhenPresent },
)?;
```

Pass the mode to `open_with_options` rather than setting it afterwards: the
Container record is decoded during `open`, so a later call would not cover it.
On the physical layer the equivalent setter is
`PhysicalParser::checksum_mode(mode)`; the older
`verify_checksums(bool)` remains as shorthand for `Always` / `Never`.

## Testing

```bash
cargo test -p pqdif
```

Tests exercise the parser against hand-constructed synthetic byte arrays; no
real `.pqd` fixtures are vendored (none exist in the upstream reference repos,
and the writer that would produce them is Phase 2). For local sanity checks
against a real file, enable the off-by-default feature:

```bash
PQDIF_TEST_FILE=/path/to/recording.pqd cargo test -p pqdif --features real-file-tests
```

The synthetic byte-array builders live in `src/test_fixtures/synthetic.rs`
behind the off-by-default `test-fixtures` feature, rather than under `tests/`,
so that dependents can drive their own tests with genuine PQDIF bytes. That is
how `minifi_pqdif` tests its processor — the alternative was a second fixture
writer maintained there, which would drift from this one. The feature is off by
default so nothing ships to real dependents; this crate's own integration tests
reach it through a dev-dependency on itself.
