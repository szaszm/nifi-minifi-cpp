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

//! Pure-Rust reader for the Power Quality Data Interchange Format
//! (PQDIF, IEEE Std 1159.3).
//!
//! The crate is layered into two APIs:
//!
//! - The [`physical`] module exposes the raw record + element tree — one
//!   [`PhysicalParser`] yields [`PhysicalRecord`]s whose bodies are element
//!   trees over the format's three element kinds
//!   ([`Element`] = Scalar / Vector / Collection).
//! - The [`logical`] module builds typed record objects on top of the
//!   physical layer: [`ContainerRecord`], [`DataSourceRecord`], and
//!   [`ObservationRecord`]. Most callers want this layer.
//!
//! # Example
//!
//! ```ignore
//! use pqdif::LogicalParser;
//! use std::fs::File;
//!
//! let file = File::open("recording.pqd")?;
//! let (mut parser, container) = LogicalParser::open(file)?;
//! println!("writer: {}", container.writer_name);
//! while let Some(obs) = parser.next_observation()? {
//!     for channel in &obs.channels {
//!         println!("channel {}: {} samples", channel.name, channel.series.len());
//!     }
//! }
//! # Ok::<(), pqdif::PqdifError>(())
//! ```
//!
//! # Deliberate deviations from the reference implementation
//!
//! - **Checksum verification is on by default.** Grid Protection Alliance's
//!   `GSF.PQDIF` computes the Adler-32 checksum on read but does not verify
//!   it. This crate does, because it is intended to process files from
//!   untrusted sources in a data-collection agent. Use
//!   [`PhysicalParser::verify_checksums`](physical::PhysicalParser::verify_checksums)
//!   to opt out.
//! - **`Read + Seek` is required.** PQDIF is a linked list of records via
//!   absolute file offsets; supporting pure `Read` would require buffering
//!   the whole file. The two calling shapes that matter — `std::fs::File`
//!   and `std::io::Cursor<&[u8]>` — both implement `Read + Seek`.
//!
//! # Reference implementation
//!
//! The binary format was ported from Grid Protection Alliance's
//! [`GSF.PQDIF`](https://github.com/GridProtectionAlliance/gsf) (C#, MIT
//! licensed) and its modernized successor
//! [`gemstone/pqdif`](https://github.com/gemstone/pqdif). This crate ships
//! a verbatim copy of GSF's `TagDefinitions.xml`; see `tags/ATTRIBUTION.md`.

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod error;
pub mod logical;
pub mod physical;
pub mod tag;
pub mod timestamp;

pub use error::PqdifError;
pub use logical::{
    ChannelInstance, ContainerRecord, DataSourceRecord, LogicalParser, ObservationRecord,
    RecordType, SeriesInstance,
};
pub use physical::{
    CollectionElement, Element, PhysicalParser, PhysicalRecord, PhysicalType, RecordHeader,
    ScalarElement, VectorElement,
};
