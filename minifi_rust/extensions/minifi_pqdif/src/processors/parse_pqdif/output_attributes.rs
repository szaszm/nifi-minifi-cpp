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

//! Attributes `ParsePqdif` writes, for the agent manifest and `PROCESSORS.md`.
//!
//! The `pqdif.*` container attributes are only written when the file actually
//! carries the corresponding tag — PQDIF marks nearly all of them optional. They
//! are all declared here regardless, because the manifest documents what a
//! processor *can* emit; a flow author needs to know the name exists before they
//! can write a `RouteOnAttribute` against it.
//!
//! `container_attributes_are_all_declared` in the tests keeps this list honest
//! against what [`crate::json::container_attributes`] actually produces.

use minifi_native::OutputAttribute;

const SUCCESS_ONLY: &[&str] = &["success"];

pub(crate) const ALL: &[OutputAttribute] = &[
    OutputAttribute {
        name: "mime.type",
        relationships: SUCCESS_ONLY,
        description: "Set to 'application/x-ndjson' or 'application/json' to match the configured \
                      Output Format",
    },
    OutputAttribute {
        name: "pqdif.observation.count",
        relationships: SUCCESS_ONLY,
        description: "Number of observations written. Zero is a valid outcome: a PQDIF file may \
                      carry only a container and data-source definition",
    },
    OutputAttribute {
        name: "pqdif.format",
        relationships: SUCCESS_ONLY,
        description: "The Output Format the content was written in, 'ndjson' or 'array', so a \
                      downstream route can pick its splitter without restating the setting",
    },
    OutputAttribute {
        name: "pqdif.writer.version",
        relationships: SUCCESS_ONLY,
        description: "PQDIF format version declared by the container record",
    },
    OutputAttribute {
        name: "pqdif.application",
        relationships: SUCCESS_ONLY,
        description: "Application that wrote the file, from the container's tagApplication",
    },
    OutputAttribute {
        name: "pqdif.author",
        relationships: SUCCESS_ONLY,
        description: "Author recorded in the container's tagAuthor",
    },
    OutputAttribute {
        name: "pqdif.source.filename",
        relationships: SUCCESS_ONLY,
        description: "Filename recorded inside the file by its writer (tagFileName). Distinct from \
                      the 'filename' attribute, which is where MiNiFi picked the file up",
    },
    OutputAttribute {
        name: "pqdif.title",
        relationships: SUCCESS_ONLY,
        description: "Container title, from tagTitle",
    },
    OutputAttribute {
        name: "pqdif.notes",
        relationships: SUCCESS_ONLY,
        description: "Free-form container notes, from tagNotes",
    },
    OutputAttribute {
        name: "pqdif.creation.time",
        relationships: SUCCESS_ONLY,
        description: "When the file was first written (tagCreation), ISO 8601 without a zone — \
                      PQDIF records no offset",
    },
    OutputAttribute {
        name: "pqdif.last.saved",
        relationships: SUCCESS_ONLY,
        description: "When the file was last rewritten (tagLastSaved), ISO 8601 without a zone",
    },
    OutputAttribute {
        name: "pqdif.times.saved",
        relationships: SUCCESS_ONLY,
        description: "How many times the file has been overwritten, from tagTimesSaved",
    },
];
