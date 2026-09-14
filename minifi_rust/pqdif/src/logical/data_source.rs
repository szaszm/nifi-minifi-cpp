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

//! Typed shape of the DataSource record.
//!
//! A DataSource describes one measurement source (an instrument or a virtual
//! stream): its name, effective time, and the list of channel definitions
//! that observations under this data source refer to by index.
//!
//! # Scope
//!
//! Phase 1 exposes the fields the reader needs to interpret Observation
//! records that reference this data source. Ancillary metadata that GSF's
//! `DataSourceRecord.cs` also carries (address, phone, email, model number,
//! monitor-side-vs-system-side transformer ratios, calibration flags) is
//! parseable via the physical layer and can be surfaced in a later phase
//! without a breaking change to this shape.

use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::error::PqdifError;
use crate::physical::element::CollectionElement;
use crate::tag::{
    TAG_CHANNEL_DEFINITIONS_TAG, TAG_CHANNEL_NAME_TAG, TAG_DATA_SOURCE_NAME_TAG,
    TAG_DATA_SOURCE_TYPE_TAG, TAG_EFFECTIVE_TAG, TAG_ONE_CHANNEL_DEFINITION_TAG,
    TAG_ONE_SERIES_DEFINITION_TAG, TAG_PHASE_ID_TAG, TAG_QUANTITY_CHARACTERISTIC_ID_TAG,
    TAG_QUANTITY_MEASURED_ID_TAG, TAG_SERIES_DEFINITIONS_TAG, TAG_VALUE_TYPE_ID_TAG,
};

use super::util::{
    collections_with_tag, decode_string, guid_from_scalar, timestamp_from_scalar, u32_or,
};

/// Typed view of a DataSource record.
#[derive(Debug, Clone)]
pub struct DataSourceRecord {
    /// Human-readable name of the data source (`tagDataSourceName`).
    pub name: Option<String>,
    /// The `tagDataSourceTypeID` GUID selecting one of
    /// `ID_DS_TYPE_MEASURE` / `MANUAL` / `SIMULATE` / `BENCHMARK` / `DEBUG`.
    pub source_type: Option<Uuid>,
    /// `tagEffective` — instant at which this data source's configuration
    /// took effect.
    pub effective_time: Option<NaiveDateTime>,
    /// Channel definitions in file order. Observation records reference
    /// these by zero-based index.
    pub channels: Vec<ChannelDefinition>,
}

impl DataSourceRecord {
    /// Build a typed record from an already-decoded record body.
    pub fn from_body(body: &CollectionElement) -> Result<Self, PqdifError> {
        let name = decode_string(body, TAG_DATA_SOURCE_NAME_TAG)?;
        let source_type = guid_from_scalar(body, TAG_DATA_SOURCE_TYPE_TAG);
        let effective_time = timestamp_from_scalar(body, TAG_EFFECTIVE_TAG)?;

        let mut channels = Vec::new();
        if let Some(defs) = body.collection(TAG_CHANNEL_DEFINITIONS_TAG) {
            for ch in collections_with_tag(defs, TAG_ONE_CHANNEL_DEFINITION_TAG) {
                channels.push(ChannelDefinition::from_body(ch)?);
            }
        }

        Ok(Self {
            name,
            source_type,
            effective_time,
            channels,
        })
    }
}

/// One channel definition inside a DataSource record.
#[derive(Debug, Clone)]
pub struct ChannelDefinition {
    /// `tagChannelName` — free-form channel label.
    pub name: Option<String>,
    /// `tagPhaseID` — phase this channel measures. See
    /// [`crate::tag::phase`].
    pub phase_id: u32,
    /// `tagQuantityMeasuredID` — the electrical quantity being measured
    /// (voltage, current, …). See [`crate::tag::quantity_measured`].
    pub quantity_measured_id: u32,
    /// Series definitions in file order.
    pub series: Vec<SeriesDefinition>,
}

impl ChannelDefinition {
    fn from_body(body: &CollectionElement) -> Result<Self, PqdifError> {
        let name = decode_string(body, TAG_CHANNEL_NAME_TAG)?;
        let phase_id = u32_or(body, TAG_PHASE_ID_TAG, crate::tag::phase::NONE);
        let quantity_measured_id = u32_or(body, TAG_QUANTITY_MEASURED_ID_TAG, crate::tag::quantity_measured::NONE);

        let mut series = Vec::new();
        if let Some(defs) = body.collection(TAG_SERIES_DEFINITIONS_TAG) {
            for s in collections_with_tag(defs, TAG_ONE_SERIES_DEFINITION_TAG) {
                series.push(SeriesDefinition::from_body(s)?);
            }
        }

        Ok(Self {
            name,
            phase_id,
            quantity_measured_id,
            series,
        })
    }
}

/// One series definition inside a channel definition.
#[derive(Debug, Clone)]
pub struct SeriesDefinition {
    /// `tagValueTypeID` — the role of this series (values, min, max, …).
    /// See [`crate::tag::series_value_type`].
    pub value_type_id: Option<Uuid>,
    /// `tagQuantityCharacteristicID` — statistical characterization
    /// (RMS, magnitude, harmonic, …). See
    /// [`crate::tag::quantity_characteristic`].
    pub quantity_characteristic_id: Option<Uuid>,
}

impl SeriesDefinition {
    fn from_body(body: &CollectionElement) -> Result<Self, PqdifError> {
        Ok(Self {
            value_type_id: guid_from_scalar(body, TAG_VALUE_TYPE_ID_TAG),
            quantity_characteristic_id: guid_from_scalar(body, TAG_QUANTITY_CHARACTERISTIC_ID_TAG),
        })
    }
}
