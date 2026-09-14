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
use crate::physical::parser::PhysicalParser;

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
        let mut physical = PhysicalParser::new(reader)?;
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

    /// Toggle checksum verification on the underlying physical parser.
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
