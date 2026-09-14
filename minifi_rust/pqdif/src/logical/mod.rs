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

//! Logical (typed record) layer of the PQDIF format.
//!
//! Builds typed record objects on top of the physical element tree. The
//! record types this crate models are the four defined by the format —
//! [`ContainerRecord`], [`DataSourceRecord`], [`ObservationRecord`], and
//! `MonitorSettings` — plus [`RecordType::Blank`], the legal-but-empty
//! sentinel that the reference implementation silently skips.
//!
//! # Phase-1 scope
//!
//! - Container: writer identity, version fields, creation/save timestamps,
//!   notes.
//! - DataSource: name, type, effective time, and its list of channel
//!   definitions with nested series definitions.
//! - Observation: name, trigger info, timestamp, per-channel instance data
//!   with decoded series values (scale + offset applied, `start/count/
//!   increment` compact form supported).
//! - MonitorSettings: parsed only far enough to identify the record type —
//!   the full typed shape is Phase 3.

pub mod container;
pub mod data_source;
pub mod observation;
pub mod parser;
pub mod record_type;
pub mod series_values;
mod util;

pub use container::ContainerRecord;
pub use data_source::{ChannelDefinition, DataSourceRecord, SeriesDefinition};
pub use observation::{ChannelInstance, ObservationRecord, SeriesInstance};
pub use parser::LogicalParser;
pub use record_type::RecordType;
