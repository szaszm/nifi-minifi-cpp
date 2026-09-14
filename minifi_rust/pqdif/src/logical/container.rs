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

//! Typed shape of the mandatory Container record.

use chrono::NaiveDateTime;

use crate::error::PqdifError;
use crate::physical::element::CollectionElement;
use crate::tag::{
    TAG_APPLICATION_TAG, TAG_AUTHOR_TAG, TAG_CREATION_TAG, TAG_FILE_NAME_TAG, TAG_LAST_SAVED_TAG,
    TAG_NOTES_TAG, TAG_TIMES_SAVED_TAG, TAG_TITLE_TAG, TAG_VERSION_INFO_TAG,
};

use super::util::{decode_string, timestamp_from_scalar};

/// Typed view of the Container record — always the first record in a PQDIF
/// file. Carries file-level identity and the compression settings that
/// govern every record after it.
#[derive(Debug, Clone, Default)]
pub struct ContainerRecord {
    /// Value of `tagVersionInfo` — the PQDIF format version this file uses
    /// (`1` in every file we have seen produced by GSF/gemstone).
    pub writer_version: u32,
    /// Application identifier from `tagApplication`, if present. The de
    /// facto "who wrote this file" string.
    pub application: Option<String>,
    /// Author name from `tagAuthor`, if present.
    pub author: Option<String>,
    /// Original filename recorded in `tagFileName`, if present.
    pub file_name: Option<String>,
    /// `tagCreation` — when the file was first written.
    pub creation_time: Option<NaiveDateTime>,
    /// `tagLastSaved` — when the file was last rewritten.
    pub last_saved: Option<NaiveDateTime>,
    /// `tagTimesSaved` — how many times the file has been overwritten.
    pub times_saved: Option<u32>,
    /// `tagTitle` — human-readable file title, if present.
    pub title: Option<String>,
    /// `tagNotes` — free-form notes, if present.
    pub notes: Option<String>,
}

impl ContainerRecord {
    /// Build a typed record from an already-decoded record body.
    pub fn from_body(body: &CollectionElement) -> Result<Self, PqdifError> {
        let writer_version = body
            .scalar(TAG_VERSION_INFO_TAG)
            .and_then(|s| s.as_u32())
            .unwrap_or(0);
        let application = decode_string(body, TAG_APPLICATION_TAG)?;
        let author = decode_string(body, TAG_AUTHOR_TAG)?;
        let file_name = decode_string(body, TAG_FILE_NAME_TAG)?;
        let creation_time = timestamp_from_scalar(body, TAG_CREATION_TAG)?;
        let last_saved = timestamp_from_scalar(body, TAG_LAST_SAVED_TAG)?;
        let times_saved = body
            .scalar(TAG_TIMES_SAVED_TAG)
            .and_then(|s| s.as_u32());
        let title = decode_string(body, TAG_TITLE_TAG)?;
        let notes = decode_string(body, TAG_NOTES_TAG)?;

        Ok(Self {
            writer_version,
            application,
            author,
            file_name,
            creation_time,
            last_saved,
            times_saved,
            title,
            notes,
        })
    }
}
