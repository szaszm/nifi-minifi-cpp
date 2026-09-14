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

//! Typed shape of the Observation record.

use chrono::NaiveDateTime;

use crate::error::PqdifError;
use crate::physical::element::CollectionElement;
use crate::tag::{
    TAG_CHANNEL_DEFINITION_INDEX_TAG, TAG_CHANNEL_INSTANCES_TAG, TAG_OBSERVATION_NAME_TAG,
    TAG_ONE_CHANNEL_INSTANCE_TAG, TAG_ONE_SERIES_INSTANCE_TAG, TAG_SERIES_INSTANCES_TAG,
    TAG_SERIES_OFFSET_TAG, TAG_SERIES_SCALE_TAG, TAG_SERIES_VALUES_TAG, TAG_STORAGE_METHOD_ID_TAG,
    TAG_TIME_CREATE_TAG, TAG_TIME_START_TAG, TAG_TIME_TRIGGERED_TAG, storage_method,
};

use super::data_source::{ChannelDefinition, DataSourceRecord, SeriesDefinition};
use super::series_values::{self, SeriesValues};
use super::util::{collections_with_tag, decode_string, timestamp_from_scalar, u32_or};

/// One measurement/event window under a data source.
#[derive(Debug, Clone)]
pub struct ObservationRecord {
    /// `tagObservationName` — human-readable observation label.
    pub name: Option<String>,
    /// `tagTimeCreate` — when the observation was constructed.
    pub time_create: Option<NaiveDateTime>,
    /// `tagTimeStart` — first sample instant.
    pub time_start: Option<NaiveDateTime>,
    /// `tagTimeTriggered` — trigger instant, if any.
    pub time_triggered: Option<NaiveDateTime>,
    /// Per-channel decoded data.
    pub channels: Vec<ChannelInstance>,
}

/// One channel-instance under an Observation.
#[derive(Debug, Clone)]
pub struct ChannelInstance {
    /// Index into the parent DataSource's `channels` list.
    pub channel_definition_index: u32,
    /// Channel name lifted from the referenced definition (if resolvable).
    pub name: Option<String>,
    /// Series instances in file order.
    pub series: Vec<SeriesInstance>,
}

/// One series-instance under a channel-instance.
#[derive(Debug, Clone)]
pub struct SeriesInstance {
    /// Decoded numeric samples, with scale + offset applied. Empty for
    /// series-share-only entries (i.e. entries that only point to a shared
    /// series in another channel).
    pub values: SeriesValues,
    /// `tagSeriesScale`, defaults to `1.0`.
    pub scale: f64,
    /// `tagSeriesOffset`, defaults to `0.0`.
    pub offset: f64,
    /// The series-definition kind copied from the parent DataSource, if the
    /// index resolved.
    pub definition: Option<SeriesDefinition>,
}

impl ObservationRecord {
    /// Build a typed record from an already-decoded record body.
    ///
    /// `data_source` — the DataSource record whose channel definitions this
    /// observation references. Pass `None` if you have not seen one yet;
    /// series values are still decoded, but channel names and series
    /// definitions cannot be filled in.
    pub fn from_body(
        body: &CollectionElement,
        data_source: Option<&DataSourceRecord>,
    ) -> Result<Self, PqdifError> {
        let name = decode_string(body, TAG_OBSERVATION_NAME_TAG)?;
        let time_create = timestamp_from_scalar(body, TAG_TIME_CREATE_TAG)?;
        let time_start = timestamp_from_scalar(body, TAG_TIME_START_TAG)?;
        let time_triggered = timestamp_from_scalar(body, TAG_TIME_TRIGGERED_TAG)?;

        let mut channels = Vec::new();
        if let Some(insts) = body.collection(TAG_CHANNEL_INSTANCES_TAG) {
            for ch in collections_with_tag(insts, TAG_ONE_CHANNEL_INSTANCE_TAG) {
                channels.push(ChannelInstance::from_body(ch, data_source)?);
            }
        }

        Ok(Self {
            name,
            time_create,
            time_start,
            time_triggered,
            channels,
        })
    }
}

impl ChannelInstance {
    fn from_body(
        body: &CollectionElement,
        data_source: Option<&DataSourceRecord>,
    ) -> Result<Self, PqdifError> {
        let channel_definition_index = body
            .scalar(TAG_CHANNEL_DEFINITION_INDEX_TAG)
            .and_then(|s| s.as_u32())
            .ok_or(PqdifError::MissingRequiredTag(TAG_CHANNEL_DEFINITION_INDEX_TAG))?;

        let channel_def: Option<&ChannelDefinition> = data_source
            .and_then(|ds| ds.channels.get(channel_definition_index as usize));

        let mut series = Vec::new();
        if let Some(insts) = body.collection(TAG_SERIES_INSTANCES_TAG) {
            for (idx, s) in
                collections_with_tag(insts, TAG_ONE_SERIES_INSTANCE_TAG).enumerate()
            {
                let series_def: Option<&SeriesDefinition> =
                    channel_def.and_then(|c| c.series.get(idx));
                series.push(SeriesInstance::from_body(s, series_def)?);
            }
        }

        Ok(Self {
            channel_definition_index,
            name: channel_def.and_then(|c| c.name.clone()),
            series,
        })
    }
}

impl SeriesInstance {
    fn from_body(
        body: &CollectionElement,
        definition: Option<&SeriesDefinition>,
    ) -> Result<Self, PqdifError> {
        let scale = body
            .scalar(TAG_SERIES_SCALE_TAG)
            .and_then(|s| s.as_f64())
            .unwrap_or(1.0);
        let offset = body
            .scalar(TAG_SERIES_OFFSET_TAG)
            .and_then(|s| s.as_f64())
            .unwrap_or(0.0);

        let storage_method = u32_or(body, TAG_STORAGE_METHOD_ID_TAG, storage_method::VALUES);
        let compact = storage_method & storage_method::INCREMENT != 0;

        let values = if let Some(values_element) = body
            .children
            .iter()
            .find(|c| c.tag() == TAG_SERIES_VALUES_TAG)
        {
            series_values::decode(values_element, scale, offset, compact)?
        } else {
            Vec::new()
        };

        Ok(Self {
            values,
            scale,
            offset,
            definition: definition.cloned(),
        })
    }
}
