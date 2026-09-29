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

//! JSON projection of the PQDIF logical model.
//!
//! The mapping lives here rather than in `pqdif` so that crate stays free of a
//! serde dependency and of any opinion about output shape. What it produces is
//! the processor's public contract, so the rules are worth stating:
//!
//! - **One JSON object per Observation record.** Observations are the natural
//!   unit: a PQDIF file is a chain of them, and they are what a downstream flow
//!   wants to route, filter, or split on.
//! - **Container metadata does not appear in the content.** It is file-level,
//!   and repeating it inside every observation would bloat the output while
//!   *still* losing it the moment someone splits. It goes onto FlowFile
//!   attributes instead ([`container_attributes`]), which MiNiFi copies onto
//!   each split FlowFile for free.
//! - **DataSource metadata is folded into the observation that references it**,
//!   not emitted as a sibling record. The data source in force can change
//!   mid-file, and its per-channel phase / measured-quantity is what makes the
//!   samples interpretable — so channel definition fields are merged onto the
//!   channel objects they describe, and only the source's name is carried at
//!   the observation level. A separate `data_sources` array would be both
//!   harder to join against and lost on split.
//! - **Enumerated ids are rendered as names where the tag registry knows one**,
//!   with the raw id kept alongside. `"phase": "AN"` is what a person reads;
//!   `"phase_id": 1` is what survives a registry that has not been taught a
//!   vendor extension. Dropping either would cost something.
//! - **Absent optional fields are omitted, not emitted as `null`.** PQDIF marks
//!   most tags optional and real files exercise that freely; `null`-padding
//!   every observation inflates a sparse file for no information gain.
//!
//! Timestamps are ISO 8601 (`%Y-%m-%dT%H:%M:%S%.f`) and naive — PQDIF stores no
//! zone, so appending `Z` would assert a UTC offset the file never claimed.

use pqdif::chrono::NaiveDateTime;
use pqdif::logical::ChannelDefinition;
use pqdif::tag::{phase, quantity_characteristic, quantity_measured, series_value_type};
use pqdif::{
    ChannelInstance, ContainerRecord, DataSourceRecord, ObservationRecord, SeriesInstance,
};
use serde_json::{Map, Value, json};

/// Attribute-name prefix for everything this module writes onto a FlowFile.
pub const ATTRIBUTE_PREFIX: &str = "pqdif.";

/// Format a naive timestamp the way every JSON field and attribute here does.
fn format_timestamp(timestamp: &NaiveDateTime) -> String {
    timestamp.format("%Y-%m-%dT%H:%M:%S%.f").to_string()
}

/// Insert `key` only if `value` is present, keeping absent fields out of the
/// object rather than emitting `null`.
fn insert_opt(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        map.insert(key.to_owned(), value);
    }
}

/// Insert an enumerated id as both its resolved name and its raw id.
///
/// `name_key` is omitted when the registry has no entry — an unknown id is
/// exactly the case where the raw value is the only thing worth reporting.
fn insert_enum(
    map: &mut Map<String, Value>,
    name_key: &str,
    id_key: &str,
    id: Value,
    name: Option<&str>,
) {
    if let Some(name) = name {
        map.insert(name_key.to_owned(), Value::String(name.to_owned()));
    }
    map.insert(id_key.to_owned(), id);
}

/// Whether sample arrays are inlined in the output.
///
/// One observation can carry hundreds of thousands of samples — a real file the
/// reader was validated against holds ~210k in a single observation — so a
/// caller that only wants the channel/series structure needs a way to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleValues {
    /// Inline every sample under `"values"`.
    #[default]
    Include,
    /// Omit `"values"`. `"sample_count"` is still emitted, so the shape of the
    /// recording survives without its bulk.
    Exclude,
}

/// Serialize one observation to a JSON object.
///
/// `index` is the observation's zero-based position in the file, carried through
/// so a split FlowFile can still be traced back to its source record.
///
/// `data_source` is whichever DataSource record was most recently in force.
/// Pass the parser's `current_data_source()`; `None` (a file whose observations
/// precede any DataSource) simply omits the fields it would have supplied.
pub fn observation_to_json(
    observation: &ObservationRecord,
    index: usize,
    data_source: Option<&DataSourceRecord>,
    samples: SampleValues,
) -> Value {
    let mut map = Map::new();
    map.insert("observation_index".to_owned(), json!(index));
    insert_opt(
        &mut map,
        "name",
        observation.name.as_ref().map(|n| json!(n)),
    );
    insert_opt(
        &mut map,
        "data_source",
        data_source
            .and_then(|ds| ds.name.as_ref())
            .map(|n| json!(n)),
    );
    insert_opt(
        &mut map,
        "time_create",
        observation.time_create.as_ref().map(format_timestamp_value),
    );
    insert_opt(
        &mut map,
        "time_start",
        observation.time_start.as_ref().map(format_timestamp_value),
    );
    insert_opt(
        &mut map,
        "time_triggered",
        observation
            .time_triggered
            .as_ref()
            .map(format_timestamp_value),
    );
    map.insert(
        "channels".to_owned(),
        Value::Array(
            observation
                .channels
                .iter()
                .map(|channel| {
                    let definition = data_source
                        .and_then(|ds| ds.channels.get(channel.channel_definition_index as usize));
                    channel_to_json(channel, definition, samples)
                })
                .collect(),
        ),
    );
    Value::Object(map)
}

fn format_timestamp_value(timestamp: &NaiveDateTime) -> Value {
    Value::String(format_timestamp(timestamp))
}

fn channel_to_json(
    channel: &ChannelInstance,
    definition: Option<&ChannelDefinition>,
    samples: SampleValues,
) -> Value {
    let mut map = Map::new();
    map.insert(
        "channel_definition_index".to_owned(),
        json!(channel.channel_definition_index),
    );
    insert_opt(&mut map, "name", channel.name.as_ref().map(|n| json!(n)));
    // Phase and measured quantity live on the DataSource's channel definition,
    // which the reader resolves the name from but does not otherwise copy over.
    // They are what identify *what* these samples measure, so they belong on
    // the channel rather than in a separate record a consumer has to join.
    if let Some(definition) = definition {
        insert_enum(
            &mut map,
            "phase",
            "phase_id",
            json!(definition.phase_id),
            phase::name(definition.phase_id),
        );
        insert_enum(
            &mut map,
            "quantity_measured",
            "quantity_measured_id",
            json!(definition.quantity_measured_id),
            quantity_measured::name(definition.quantity_measured_id),
        );
    }
    map.insert(
        "series".to_owned(),
        Value::Array(
            channel
                .series
                .iter()
                .map(|series| series_to_json(series, samples))
                .collect(),
        ),
    );
    Value::Object(map)
}

fn series_to_json(series: &SeriesInstance, samples: SampleValues) -> Value {
    let mut map = Map::new();
    if let Some(definition) = &series.definition {
        if let Some(id) = definition.value_type_id {
            insert_enum(
                &mut map,
                "value_type",
                "value_type_id",
                json!(id.to_string()),
                series_value_type::name(id),
            );
        }
        if let Some(id) = definition.quantity_characteristic_id {
            insert_enum(
                &mut map,
                "quantity_characteristic",
                "quantity_characteristic_id",
                json!(id.to_string()),
                quantity_characteristic::name(id),
            );
        }
    }
    // Scale and offset are already folded into `values` by the reader. They are
    // reported anyway because they are part of how the file encoded the data,
    // and a consumer reconciling output against the original file needs them.
    map.insert("scale".to_owned(), json!(series.scale));
    map.insert("offset".to_owned(), json!(series.offset));
    map.insert("sample_count".to_owned(), json!(series.values.len()));
    if samples == SampleValues::Include {
        map.insert("values".to_owned(), json!(series.values));
    }
    Value::Object(map)
}

/// File-level Container metadata as `(attribute name, value)` pairs.
///
/// Attributes rather than content, because MiNiFi copies attributes onto every
/// FlowFile derived from this one — so `pqdif.application` still identifies the
/// writer after a downstream split has fanned the observations out, which is
/// not true of anything embedded in the content.
pub fn container_attributes(container: &ContainerRecord) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    let mut push = |key: &str, value: String| {
        attributes.push((format!("{ATTRIBUTE_PREFIX}{key}"), value));
    };

    push("writer.version", container.writer_version.to_string());
    if let Some(application) = &container.application {
        push("application", application.clone());
    }
    if let Some(author) = &container.author {
        push("author", author.clone());
    }
    if let Some(file_name) = &container.file_name {
        push("source.filename", file_name.clone());
    }
    if let Some(title) = &container.title {
        push("title", title.clone());
    }
    if let Some(notes) = &container.notes {
        push("notes", notes.clone());
    }
    if let Some(creation_time) = &container.creation_time {
        push("creation.time", format_timestamp(creation_time));
    }
    if let Some(last_saved) = &container.last_saved {
        push("last.saved", format_timestamp(last_saved));
    }
    if let Some(times_saved) = container.times_saved {
        push("times.saved", times_saved.to_string());
    }
    attributes
}

#[cfg(test)]
mod tests;
