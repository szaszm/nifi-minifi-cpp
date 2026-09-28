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

//! Iterator-shaped logical-layer parser.
//!
//! Reads the Container eagerly on [`LogicalParser::open`] so callers can
//! branch on writer / version / creation-time immediately. Subsequent calls
//! to [`LogicalParser::next_observation`] walk the record chain, updating
//! the "last-seen DataSource" as they go and returning Observation records
//! resolved against it. Blank and Unknown records are silently skipped, in
//! keeping with the reference implementation.

use std::io::{Read, Seek};

use crate::error::PqdifError;
use crate::logical::container::ContainerRecord;
use crate::logical::data_source::DataSourceRecord;
use crate::logical::observation::ObservationRecord;
use crate::logical::record_type::RecordType;
use crate::physical::checksum::ChecksumMode;
use crate::physical::parser::PhysicalParser;

/// Options controlling how [`LogicalParser::open_with_options`] reads a file.
///
/// Construct from [`Default`] and override what you need:
///
/// ```
/// use pqdif::{ChecksumMode, OpenOptions};
///
/// let lenient = OpenOptions {
///     checksum_mode: ChecksumMode::WhenPresent,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions {
    /// When to verify the Adler-32 checksum in each record's header against
    /// the on-disk body bytes; a mismatch raises
    /// [`PqdifError::ChecksumMismatch`]. Defaults to
    /// [`ChecksumMode::Always`]. Use [`ChecksumMode::WhenPresent`] to accept
    /// files from writers that leave the field zero while still checking
    /// those that populate it.
    pub checksum_mode: ChecksumMode,
}

/// Iterator over the typed records inside a PQDIF file.
pub struct LogicalParser<R: Read + Seek> {
    physical: PhysicalParser<R>,
    /// The most recently seen DataSource, used to resolve subsequent
    /// Observation records. A file may switch data sources mid-stream by
    /// emitting a new DataSource record; each Observation picks up whichever
    /// definition was most recently declared.
    current_data_source: Option<DataSourceRecord>,
}

impl<R: Read + Seek> LogicalParser<R> {
    /// Open a PQDIF file and return the parser plus the (mandatory)
    /// Container record.
    ///
    /// Fails if the file's first record is not a Container.
    pub fn open(reader: R) -> Result<(Self, ContainerRecord), PqdifError> {
        Self::open_with_options(reader, OpenOptions::default())
    }

    /// Like [`Self::open`], but applies `options` to the underlying physical
    /// parser before reading the Container record.
    ///
    /// Use this to open files that ship with a zero Adler-32 checksum — a
    /// common quirk of third-party writers (the reference GSF implementation
    /// computes but does not verify on read). Constructing a `LogicalParser`
    /// via [`Self::open`] and then calling [`Self::checksum_mode`] is too
    /// late for the Container record, which is decoded during `open`.
    pub fn open_with_options(
        reader: R,
        options: OpenOptions,
    ) -> Result<(Self, ContainerRecord), PqdifError> {
        let mut physical = PhysicalParser::new(reader)?;
        physical.checksum_mode(options.checksum_mode);
        let first = physical
            .next_record()?
            .ok_or(PqdifError::BadHeader("file has no records"))?;
        if RecordType::from_type_tag(first.header.type_tag) != RecordType::Container {
            return Err(PqdifError::BadHeader(
                "first record must be a Container",
            ));
        }
        let container = ContainerRecord::from_body(&first.body)?;
        Ok((
            Self {
                physical,
                current_data_source: None,
            },
            container,
        ))
    }

    /// Set the checksum verification policy on the underlying physical
    /// parser, for the records read from here on.
    ///
    /// The Container record is already decoded by the time you can call
    /// this; pass a mode to [`Self::open_with_options`] to cover it too.
    pub fn checksum_mode(&mut self, mode: ChecksumMode) -> &mut Self {
        self.physical.checksum_mode(mode);
        self
    }

    /// Toggle checksum verification on the underlying physical parser.
    ///
    /// Shorthand for [`ChecksumMode::Always`] / [`ChecksumMode::Never`]; use
    /// [`Self::checksum_mode`] to reach [`ChecksumMode::WhenPresent`].
    pub fn verify_checksums(&mut self, verify: bool) -> &mut Self {
        self.physical.verify_checksums(verify);
        self
    }

    /// Return the current DataSource record, if any has been seen.
    pub fn current_data_source(&self) -> Option<&DataSourceRecord> {
        self.current_data_source.as_ref()
    }

    /// Return the next Observation record.
    ///
    /// DataSource records encountered along the way are stored internally
    /// and used to resolve subsequent observations. Blank / Unknown records
    /// are skipped. Returns `Ok(None)` when the record chain ends.
    pub fn next_observation(&mut self) -> Result<Option<ObservationRecord>, PqdifError> {
        while let Some(record) = self.physical.next_record()? {
            match RecordType::from_type_tag(record.header.type_tag) {
                RecordType::DataSource => {
                    self.current_data_source =
                        Some(DataSourceRecord::from_body(&record.body)?);
                }
                RecordType::Observation => {
                    let obs = ObservationRecord::from_body(
                        &record.body,
                        self.current_data_source.as_ref(),
                    )?;
                    return Ok(Some(obs));
                }
                RecordType::Container => {
                    // Files should only carry one Container, at offset 0.
                    // A second Container is a spec violation but not a
                    // parsing error — just skip it, matching GSF.
                }
                RecordType::MonitorSettings
                | RecordType::Blank
                | RecordType::Unknown(_) => {
                    // Phase-1 scope: MonitorSettings is not typed;
                    // Blank/Unknown are silently skipped by design.
                }
            }
        }
        Ok(None)
    }
}
