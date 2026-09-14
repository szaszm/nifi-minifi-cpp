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

//! PQDIF tag registry.
//!
//! Every constant here is generated at build time from
//! `tags/tag_definitions.xml` (vendored verbatim from Grid Protection
//! Alliance's GSF.PQDIF, MIT licensed — see `tags/ATTRIBUTION.md`).
//!
//! Structural tags identify elements inside a record's element tree; they
//! live directly under this module as `TAG_<NAME>: Uuid`. The enumerated
//! tag-value groups the reader consumes are exposed as nested modules:
//!
//! - [`compression_style`] — `<tagCompressionStyleID>` (`u32`).
//! - [`compression_algorithm`] — `<tagCompressionAlgorithmID>` (`u32`).
//! - [`phase`] — `<tagPhaseID>` (`u32`).
//! - [`series_value_type`] — `<tagValueTypeID>` (GUID).
//! - [`quantity_characteristic`] — `<tagQuantityCharacteristicID>` (GUID).
//! - [`quantity_measured`] — `<tagQuantityMeasuredID>` (GUID).

// The generated file embeds description text from tag_definitions.xml
// verbatim, some of which contains bare URLs. Silence the rustdoc bare-URL
// warning for the generated content only.
#[allow(rustdoc::bare_urls)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_tags.rs"));
}
pub use generated::*;

/// Look up a structural tag by its GUID.
pub fn find(id: uuid::Uuid) -> Option<&'static TagInfo> {
    TAGS.iter().find(|t| t.id == id)
}
