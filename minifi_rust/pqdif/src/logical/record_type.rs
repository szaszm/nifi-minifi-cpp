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

//! Enumeration of the five record kinds identified by the record header's
//! `type_tag` GUID.

use uuid::Uuid;

use crate::tag::{
    TAG_BLANK_TAG, TAG_CONTAINER_TAG, TAG_DATA_SOURCE_TAG, TAG_MONITOR_SETTINGS_TAG,
    TAG_OBSERVATION_TAG,
};

/// The five record-level GUIDs defined by PQDIF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    /// The mandatory first record of any file — carries version identity
    /// and compression settings.
    Container,
    /// Describes a measurement source (channels, series shapes, effective
    /// time). May appear more than once.
    DataSource,
    /// Optional configuration snapshot for a monitor. Phase-1 exposes only
    /// the discriminant; typed fields are Phase 3.
    MonitorSettings,
    /// One recorded event or measurement window.
    Observation,
    /// Placeholder record — legal but skipped by the reference
    /// implementation and by [`crate::LogicalParser`].
    Blank,
    /// An unknown record kind. GSF ignores such records to remain forward
    /// compatible; we preserve the raw GUID for callers who need it.
    Unknown(Uuid),
}

impl RecordType {
    /// Classify a record header's `type_tag`.
    pub fn from_type_tag(tag: Uuid) -> Self {
        match tag {
            t if t == TAG_CONTAINER_TAG => Self::Container,
            t if t == TAG_DATA_SOURCE_TAG => Self::DataSource,
            t if t == TAG_MONITOR_SETTINGS_TAG => Self::MonitorSettings,
            t if t == TAG_OBSERVATION_TAG => Self::Observation,
            t if t == TAG_BLANK_TAG => Self::Blank,
            other => Self::Unknown(other),
        }
    }
}
