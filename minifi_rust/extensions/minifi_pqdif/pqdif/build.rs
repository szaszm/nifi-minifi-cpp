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

//! Build-time generator for the PQDIF tag registry.
//!
//! Parses `tags/tag_definitions.xml` (vendored verbatim from Grid Protection
//! Alliance's GSF.PQDIF) and emits `${OUT_DIR}/generated_tags.rs` containing:
//!
//! - One `pub const TAG_<UPPER_SNAKE>: Uuid = uuid!("…");` per structural
//!   tag under `<tagInfo>`.
//! - A `pub static TAGS: &[TagInfo]` slice for reflection.
//! - Modules with constants for the tag-value groups the reader consumes:
//!   `compression_style`, `compression_algorithm`, `phase`,
//!   `series_value_type`, `quantity_characteristic`, `quantity_measured`.
//!   Other tag-value groups (vendor IDs, equipment IDs, disturbance
//!   categories, etc.) are Phase 3 scope and are skipped by this generator.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use xml::reader::{EventReader, XmlEvent};

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let xml_path = manifest_dir.join("tags").join("tag_definitions.xml");
    println!("cargo:rerun-if-changed={}", xml_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let out_path = out_dir.join("generated_tags.rs");

    let doc = Document::parse(&xml_path);
    fs::write(&out_path, doc.render()).expect("write generated_tags.rs");
}

struct Document {
    structural: Vec<StructuralTag>,
    numeric_groups: Vec<NumericGroup>,
    guid_groups: Vec<GuidGroup>,
}

#[derive(Default)]
struct StructuralTag {
    id: String,
    name: String,
    standard_name: String,
    description: String,
    element_type: String,
    physical_type: String,
    required: String,
}

struct NumericGroup {
    xml_name: String,
    entries: Vec<NumericEntry>,
}

#[derive(Default)]
struct NumericEntry {
    name: String,
    standard_name: String,
    value: u32,
    description: String,
}

struct GuidGroup {
    xml_name: String,
    entries: Vec<GuidEntry>,
}

#[derive(Default)]
struct GuidEntry {
    name: String,
    standard_name: String,
    value: String,
    description: String,
}

/// XML tag-value groups whose values are small non-negative integers.
const NUMERIC_GROUP_NAMES: &[&str] = &[
    "tagCompressionStyleID",
    "tagCompressionAlgorithmID",
    "tagPhaseID",
    "tagStorageMethodID",
    "tagQuantityMeasuredID",
];

/// XML tag-value groups whose values are GUIDs. Only the ones the reader
/// consumes in Phase 1; the remaining vendor/equipment/etc. groups are
/// Phase 3 scope and are skipped by this generator.
const GUID_GROUP_NAMES: &[&str] = &["tagValueTypeID", "tagQuantityCharacteristicID"];

impl Document {
    fn parse(path: &Path) -> Self {
        let file = fs::File::open(path).expect("open tag_definitions.xml");
        let reader = EventReader::new(BufReader::new(file));

        let mut stack: Vec<String> = Vec::new();
        let mut structural: Vec<StructuralTag> = Vec::new();
        let mut numeric_groups: Vec<NumericGroup> = Vec::new();
        let mut guid_groups: Vec<GuidGroup> = Vec::new();

        let mut cur_struct: Option<StructuralTag> = None;
        let mut cur_num_entry: Option<NumericEntry> = None;
        let mut cur_guid_entry: Option<GuidEntry> = None;
        let mut cur_num_group: Option<NumericGroup> = None;
        let mut cur_guid_group: Option<GuidGroup> = None;
        let mut cur_text = String::new();

        for ev in reader {
            let ev = ev.expect("well-formed tag_definitions.xml");
            match ev {
                XmlEvent::StartElement { name, .. } => {
                    let local = name.local_name;
                    stack.push(local);
                    cur_text.clear();

                    if is_path(&stack, &["TagDefinitions", "tagInfo", "tag"]) {
                        cur_struct = Some(StructuralTag::default());
                    }

                    if stack.len() == 3 && stack[0] == "TagDefinitions" && stack[1] == "tagValues" {
                        let group = stack[2].as_str();
                        if NUMERIC_GROUP_NAMES.contains(&group) {
                            cur_num_group = Some(NumericGroup {
                                xml_name: group.to_string(),
                                entries: Vec::new(),
                            });
                        } else if GUID_GROUP_NAMES.contains(&group) {
                            cur_guid_group = Some(GuidGroup {
                                xml_name: group.to_string(),
                                entries: Vec::new(),
                            });
                        }
                    }

                    if stack.len() == 4
                        && stack[0] == "TagDefinitions"
                        && stack[1] == "tagValues"
                        && stack[3] == "id"
                    {
                        let group = stack[2].as_str();
                        if NUMERIC_GROUP_NAMES.contains(&group) {
                            cur_num_entry = Some(NumericEntry::default());
                        } else if GUID_GROUP_NAMES.contains(&group) {
                            cur_guid_entry = Some(GuidEntry::default());
                        }
                    }
                }
                XmlEvent::Characters(text) | XmlEvent::CData(text) => {
                    cur_text.push_str(&text);
                }
                XmlEvent::EndElement { name } => {
                    let local = name.local_name;

                    // Populate the appropriate accumulator before popping.
                    if let Some(t) = cur_struct.as_mut()
                        && stack.len() == 4
                        && stack[0] == "TagDefinitions"
                        && stack[1] == "tagInfo"
                        && stack[2] == "tag"
                        && stack[3] == local
                    {
                        let text = cur_text.trim().to_string();
                        match local.as_str() {
                            "id" => t.id = text,
                            "name" => t.name = text,
                            "standardName" => t.standard_name = text,
                            "description" => t.description = text,
                            "elementType" => t.element_type = text,
                            "physicalType" => t.physical_type = text,
                            "required" => t.required = text,
                            _ => {}
                        }
                    }

                    if let Some(e) = cur_num_entry.as_mut()
                        && stack.len() == 5
                        && stack[0] == "TagDefinitions"
                        && stack[1] == "tagValues"
                        && stack[3] == "id"
                        && stack[4] == local
                    {
                        let text = cur_text.trim().to_string();
                        match local.as_str() {
                            "name" => e.name = text,
                            "standardName" => e.standard_name = text,
                            "value" => {
                                // Values are stored either as decimal ("0", "1", …) or
                                // hex-prefixed ("0x01") depending on the tag group.
                                let parsed = if let Some(rest) =
                                    text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
                                {
                                    u32::from_str_radix(rest, 16).ok()
                                } else {
                                    text.parse::<u32>().ok()
                                };
                                e.value = parsed.unwrap_or_else(|| {
                                    panic!(
                                        "non-numeric <value> {text:?} inside numeric group {}",
                                        stack[2]
                                    )
                                });
                            }
                            "description" => e.description = text,
                            _ => {}
                        }
                    }

                    if let Some(e) = cur_guid_entry.as_mut()
                        && stack.len() == 5
                        && stack[0] == "TagDefinitions"
                        && stack[1] == "tagValues"
                        && stack[3] == "id"
                        && stack[4] == local
                    {
                        let text = cur_text.trim().to_string();
                        match local.as_str() {
                            "name" => e.name = text,
                            "standardName" => e.standard_name = text,
                            "value" => e.value = text,
                            "description" => e.description = text,
                            _ => {}
                        }
                    }

                    // Commit on the closing tag of each enclosing element.
                    if is_path(&stack, &["TagDefinitions", "tagInfo", "tag"])
                        && local == "tag"
                        && let Some(t) = cur_struct.take()
                    {
                        structural.push(t);
                    }

                    if stack.len() == 4
                        && stack[0] == "TagDefinitions"
                        && stack[1] == "tagValues"
                        && stack[3] == "id"
                        && local == "id"
                    {
                        if let Some(entry) = cur_num_entry.take()
                            && let Some(group) = cur_num_group.as_mut()
                        {
                            group.entries.push(entry);
                        }
                        if let Some(entry) = cur_guid_entry.take()
                            && let Some(group) = cur_guid_group.as_mut()
                        {
                            group.entries.push(entry);
                        }
                    }

                    if stack.len() == 3
                        && stack[0] == "TagDefinitions"
                        && stack[1] == "tagValues"
                        && stack[2] == local
                    {
                        if let Some(group) = cur_num_group.take() {
                            numeric_groups.push(group);
                        }
                        if let Some(group) = cur_guid_group.take() {
                            guid_groups.push(group);
                        }
                    }

                    stack.pop();
                    cur_text.clear();
                }
                _ => {}
            }
        }

        Self {
            structural,
            numeric_groups,
            guid_groups,
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("// @generated by build.rs from tags/tag_definitions.xml — do not edit.\n\n");
        out.push_str("use uuid::{Uuid, uuid};\n\n");

        // Reflection type.
        out.push_str("/// Metadata for one PQDIF structural tag.\n");
        out.push_str("#[derive(Debug, Clone, Copy)]\n");
        out.push_str("pub struct TagInfo {\n");
        out.push_str("    /// GUID that identifies the tag on the wire.\n");
        out.push_str("    pub id: Uuid,\n");
        out.push_str("    /// Short human-readable name from `<name>`.\n");
        out.push_str("    pub name: &'static str,\n");
        out.push_str("    /// The `tagFoo` identifier used by the standard.\n");
        out.push_str("    pub standard_name: &'static str,\n");
        out.push_str("    /// Free-form description from the reference XML.\n");
        out.push_str("    pub description: &'static str,\n");
        out.push_str("    /// One of `Scalar`, `Vector`, `Collection`.\n");
        out.push_str("    pub element_type: &'static str,\n");
        out.push_str(
            "    /// Physical-type name (e.g. `UnsignedInteger4`), or `0` for containers.\n",
        );
        out.push_str("    pub physical_type: &'static str,\n");
        out.push_str("    /// `True` iff the tag is required by the spec at its position.\n");
        out.push_str("    pub required: &'static str,\n");
        out.push_str("}\n\n");

        // Per-tag constants.
        for t in &self.structural {
            let const_name = to_screaming_snake(&t.name);
            out.push_str(&format!(
                "/// {desc}\n///\n/// Standard name: `{sn}`. Element type: `{et}`.\n",
                desc = escape_for_doc(&t.description),
                sn = t.standard_name,
                et = t.element_type,
            ));
            out.push_str(&format!(
                "pub const TAG_{const_name}: Uuid = uuid!(\"{id}\");\n\n",
                id = t.id,
            ));
        }

        // Reflection slice.
        out.push_str("/// Every structural tag defined in `tag_definitions.xml`, in file order.\n");
        out.push_str("pub static TAGS: &[TagInfo] = &[\n");
        for t in &self.structural {
            out.push_str("    TagInfo {\n");
            out.push_str(&format!("        id: uuid!(\"{}\"),\n", t.id));
            out.push_str(&format!("        name: {:?},\n", t.name));
            out.push_str(&format!("        standard_name: {:?},\n", t.standard_name));
            out.push_str(&format!("        description: {:?},\n", t.description));
            out.push_str(&format!("        element_type: {:?},\n", t.element_type));
            out.push_str(&format!("        physical_type: {:?},\n", t.physical_type));
            out.push_str(&format!("        required: {:?},\n", t.required));
            out.push_str("    },\n");
        }
        out.push_str("];\n\n");

        for g in &self.numeric_groups {
            let mod_name = numeric_module_name(&g.xml_name);
            out.push_str(&format!(
                "/// Enumeration values for the `{}` tag-value group.\n",
                g.xml_name
            ));
            out.push_str(&format!("pub mod {mod_name} {{\n"));
            for e in &g.entries {
                let cn = to_screaming_snake(&e.name);
                out.push_str(&format!(
                    "    /// {}\n    ///\n    /// Standard name: `{}`.\n    pub const {cn}: u32 = {};\n",
                    escape_for_doc(&e.description),
                    e.standard_name,
                    e.value,
                ));
            }
            out.push_str(&render_name_table(
                "u32",
                g.entries
                    .iter()
                    .map(|e| (e.value.to_string(), e.name.as_str())),
            ));
            out.push_str("}\n\n");
        }

        for g in &self.guid_groups {
            let mod_name = guid_module_name(&g.xml_name);
            out.push_str(&format!(
                "/// Enumeration values for the `{}` tag-value group.\n",
                g.xml_name
            ));
            out.push_str(&format!("pub mod {mod_name} {{\n"));
            out.push_str("    use uuid::{Uuid, uuid};\n");
            for e in &g.entries {
                let cn = to_screaming_snake(&e.name);
                out.push_str(&format!(
                    "    /// {}\n    ///\n    /// Standard name: `{}`.\n    pub const {cn}: Uuid = uuid!({:?});\n",
                    escape_for_doc(&e.description),
                    e.standard_name,
                    e.value,
                ));
            }
            out.push_str(&render_name_table(
                "Uuid",
                g.entries
                    .iter()
                    .map(|e| (format!("uuid!({:?})", e.value), e.name.as_str())),
            ));
            out.push_str("}\n\n");
        }

        out
    }
}

/// Render the `NAMES` table and `name()` lookup appended to every tag-value
/// group module.
///
/// The reverse direction (value → human-readable name) is what a serializer
/// needs: files carry the numeric / GUID id, and emitting `"AN"` rather than
/// `1` is the difference between output a person can read and output they have
/// to cross-reference against the standard. `ty` is the group's value type,
/// and `entries` yields `(rust literal for the value, name)` pairs.
///
/// Values are unique within each group in the XML as it stands, but `name()`
/// is specified as "first match in file order" so that an alias added upstream
/// changes what the lookup returns rather than making it unspecified.
fn render_name_table<'a>(ty: &str, entries: impl Iterator<Item = (String, &'a str)>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n    /// Every entry in this group as `(value, name)`, in the order \
         `tag_definitions.xml`\n    /// declares them.\n    pub static NAMES: \
         &[({ty}, &str)] = &[\n",
    ));
    for (value, name) in entries {
        out.push_str(&format!("        ({value}, {name:?}),\n"));
    }
    out.push_str("    ];\n\n");
    out.push_str(&format!(
        "    /// Human-readable name for `value`, or `None` if this group does \
         not define it.\n    ///\n    /// Returns the first match in file \
         order.\n    pub fn name(value: {ty}) -> Option<&'static str> {{\n     \
         \x20  NAMES.iter().find(|(v, _)| *v == value).map(|(_, n)| *n)\n    \
         }}\n",
    ));
    out
}

fn is_path(stack: &[String], path: &[&str]) -> bool {
    stack.len() == path.len() && stack.iter().zip(path.iter()).all(|(a, b)| a == b)
}

/// Convert a mixed-case identifier such as `QuantityMeasured` or `PF3PhaseAvg`
/// into `QUANTITY_MEASURED` / `PF3_PHASE_AVG`. Non-ASCII-alphanumeric
/// characters become underscores; runs of underscores collapse; a leading
/// digit is prefixed with `_` so the result is a legal Rust identifier.
fn to_screaming_snake(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    let chars: Vec<char> = input.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_alphanumeric() {
            let prev_lower_or_digit =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let prev_upper_next_lower = i > 1
                && chars[i - 1].is_ascii_uppercase()
                && c.is_ascii_uppercase()
                && chars.get(i + 1).is_some_and(|nx| nx.is_ascii_lowercase());
            if i > 0 && c.is_ascii_uppercase() && (prev_lower_or_digit || prev_upper_next_lower) {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn numeric_module_name(xml: &str) -> &'static str {
    match xml {
        "tagCompressionStyleID" => "compression_style",
        "tagCompressionAlgorithmID" => "compression_algorithm",
        "tagPhaseID" => "phase",
        "tagStorageMethodID" => "storage_method",
        "tagQuantityMeasuredID" => "quantity_measured",
        other => panic!("unknown numeric tagValues group {other}"),
    }
}

fn guid_module_name(xml: &str) -> &'static str {
    match xml {
        "tagValueTypeID" => "series_value_type",
        "tagQuantityCharacteristicID" => "quantity_characteristic",
        other => panic!("unknown guid tagValues group {other}"),
    }
}

fn escape_for_doc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            in_ws = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}
