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
//! - [`storage_method`] — `<tagStorageMethodID>` (`u32`, a bitmask).
//! - [`quantity_measured`] — `<tagQuantityMeasuredID>` (`u32`).
//! - [`series_value_type`] — `<tagValueTypeID>` (GUID).
//! - [`quantity_characteristic`] — `<tagQuantityCharacteristicID>` (GUID).
//!
//! Each of those modules also carries a `NAMES` table and a `name(value)`
//! lookup for the reverse direction. Files store only the id, so anything
//! rendering PQDIF data for a human — or serializing it to JSON — needs the
//! id-to-name mapping the XML supplies:
//!
//! ```
//! use pqdif::tag::{phase, quantity_characteristic};
//!
//! assert_eq!(phase::name(phase::AN), Some("AN"));
//! assert_eq!(phase::name(9999), None);
//! assert_eq!(
//!     quantity_characteristic::name(quantity_characteristic::RMS),
//!     Some("RMS"),
//! );
//! ```

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The `name()` lookups are generated, so this pins the generator rather
    /// than the data: one numeric group and one GUID group, each checked for a
    /// hit, a miss, and agreement with the corresponding constant.
    #[test]
    fn numeric_group_names_resolve() {
        assert_eq!(phase::name(phase::NONE), Some("None"));
        assert_eq!(phase::name(phase::AN), Some("AN"));
        assert_eq!(phase::name(u32::MAX), None);
    }

    #[test]
    fn guid_group_names_resolve() {
        assert_eq!(
            series_value_type::name(series_value_type::VALUES),
            Some("Values"),
        );
        assert_eq!(series_value_type::name(uuid::Uuid::nil()), None);
    }

    #[test]
    fn names_table_covers_every_entry_it_declares() {
        // Every listed value must be findable; catches a generator that emits
        // the table and the lookup out of sync.
        for (value, name) in phase::NAMES {
            assert_eq!(phase::name(*value), Some(*name));
        }
        for (value, name) in quantity_characteristic::NAMES {
            assert_eq!(quantity_characteristic::name(*value), Some(*name));
        }
    }
}
