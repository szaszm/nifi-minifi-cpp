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

//! `ParsePqdif` — read a PQDIF recording from FlowFile content and emit its
//! observations as JSON.
//!
//! One FlowFile in, one FlowFile out. The alternative — a FlowFile per
//! observation — was deliberately not taken: files routinely hold hundreds of
//! observations, and fanning those out unconditionally puts that multiplier on
//! every repository write and queue operation downstream. Both output formats
//! here are splittable by a standard processor, so a flow that *wants* the
//! fan-out can ask for it explicitly and pay for it there:
//!
//! - `ndjson` (default) → `SplitText` with `Line Split Count` = 1.
//! - `array` → `SplitJson` with a `JsonPath Expression` of `$[*]`.
//!
//! The whole file is buffered, which is forced rather than chosen: PQDIF is a
//! linked list of records addressed by absolute file offset, so the parser needs
//! `Read + Seek`, while the API's `InputStream` is `BufRead` only. Content is
//! read to a `Vec<u8>` and wrapped in a `Cursor`.

use crate::json::{self, SampleValues};
use minifi_native::macros::{ComponentIdentifier, PropertyType};
use minifi_native::{
    FlowFileTransform, GetAttribute, GetControllerService, GetId, GetProperty, InputStream,
    LogLevel, Logger, MinifiError, ProcessError, RouteErrorExt, Schedule, TransformedFlowFile,
    debug,
};
use pqdif::{ChecksumMode, LogicalParser, OpenOptions};
use std::io::Cursor;
use strum_macros::{Display, EnumString, IntoStaticStr, VariantNames};

use relationships::{FAILURE, SUCCESS};

mod output_attributes;
mod properties;
mod relationships;

/// Framing for the per-observation JSON objects.
///
/// The objects themselves are identical between the two; only the separators
/// differ, so switching format does not change any consumer's field paths.
#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "camelCase", const_into_str)]
pub(crate) enum OutputFormat {
    /// Newline-delimited JSON: one object per line, no enclosing array.
    Ndjson,
    /// A single JSON array of objects.
    Array,
}

/// Property-facing spelling of [`ChecksumMode`].
///
/// A distinct enum rather than reusing `ChecksumMode` directly, because the
/// property layer needs `strum` derives and `pqdif` is deliberately free of
/// that dependency.
#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "camelCase", const_into_str)]
pub(crate) enum ChecksumVerification {
    /// Verify every record; a writer that left the field zero is rejected.
    Always,
    /// Verify records whose checksum is non-zero, accept those that are zero.
    WhenPresent,
    /// Skip verification entirely.
    Never,
}

impl From<ChecksumVerification> for ChecksumMode {
    fn from(value: ChecksumVerification) -> Self {
        match value {
            ChecksumVerification::Always => ChecksumMode::Always,
            ChecksumVerification::WhenPresent => ChecksumMode::WhenPresent,
            ChecksumVerification::Never => ChecksumMode::Never,
        }
    }
}

#[derive(Debug, ComponentIdentifier)]
pub(crate) struct ParsePqdif {
    output_format: OutputFormat,
    checksum_mode: ChecksumMode,
    sample_values: SampleValues,
}

/// Attribute naming the number of observations serialized.
const OBSERVATION_COUNT_ATTRIBUTE: &str = "pqdif.observation.count";
/// Attribute naming the output framing, so a downstream route can pick its
/// splitter without the flow author restating the format.
const FORMAT_ATTRIBUTE: &str = "pqdif.format";
const MIME_TYPE_ATTRIBUTE: &str = "mime.type";

/// What [`ParsePqdif::render`] produces on success.
struct Rendered {
    body: Vec<u8>,
    observation_count: usize,
    container_attributes: Vec<(String, String)>,
}

/// Anything that went wrong while reading the file. Boxed rather than a bespoke
/// enum because the two sources ([`pqdif::PqdifError`] and `serde_json::Error`)
/// are both reported the same way — logged and routed to `failure` — so a type
/// that distinguishes them would never be matched on. `RouteErrorExt` boxes it
/// at the point of use anyway.
type RenderError = Box<dyn std::error::Error + Send + Sync + 'static>;

impl ParsePqdif {
    /// Parse `bytes` and render the observations in the configured framing.
    fn render(&self, bytes: Vec<u8>) -> Result<Rendered, RenderError> {
        let (mut parser, container) = LogicalParser::open_with_options(
            Cursor::new(bytes),
            OpenOptions {
                checksum_mode: self.checksum_mode,
            },
        )?;
        let container_attributes = json::container_attributes(&container);

        let mut body = Vec::new();
        let mut observation_count = 0usize;
        if self.output_format == OutputFormat::Array {
            body.push(b'[');
        }
        while let Some(observation) = parser.next_observation()? {
            let value = json::observation_to_json(
                &observation,
                observation_count,
                parser.current_data_source(),
                self.sample_values,
            );
            if self.output_format == OutputFormat::Array && observation_count > 0 {
                body.push(b',');
            }
            serde_json::to_writer(&mut body, &value)?;
            if self.output_format == OutputFormat::Ndjson {
                body.push(b'\n');
            }
            observation_count += 1;
        }
        if self.output_format == OutputFormat::Array {
            body.push(b']');
        }
        Ok(Rendered {
            body,
            observation_count,
            container_attributes,
        })
    }

    fn mime_type(&self) -> &'static str {
        match self.output_format {
            // There is no registered media type for newline-delimited JSON;
            // application/x-ndjson is the de facto one, and NiFi's own
            // record writers use it.
            OutputFormat::Ndjson => "application/x-ndjson",
            OutputFormat::Array => "application/json",
        }
    }
}

impl Schedule for ParsePqdif {
    fn schedule<Ctx: GetProperty, L: Logger>(
        context: &Ctx,
        _logger: &L,
    ) -> Result<Self, MinifiError> {
        let include_samples = context.get_property(&properties::INCLUDE_SAMPLE_VALUES)?;
        Ok(Self {
            output_format: context.get_property(&properties::OUTPUT_FORMAT)?,
            checksum_mode: context
                .get_property(&properties::CHECKSUM_VERIFICATION)?
                .into(),
            sample_values: if include_samples {
                SampleValues::Include
            } else {
                SampleValues::Exclude
            },
        })
    }
}

impl FlowFileTransform for ParsePqdif {
    fn transform<
        'a,
        Context: GetProperty + GetControllerService + GetAttribute + GetId,
        LoggerImpl: Logger,
    >(
        &self,
        _context: &Context,
        input_stream: &'a mut dyn InputStream,
        logger: &LoggerImpl,
    ) -> Result<TransformedFlowFile<'a>, ProcessError> {
        // PQDIF needs Seek; see the module comment. A failure reading the
        // session's own content stream is an engine-level problem, not bad
        // input, so it stays fatal rather than routing to `failure`.
        let mut bytes = Vec::new();
        input_stream.read_to_end(&mut bytes)?;

        // A file we cannot read is this FlowFile's problem, not the processor's:
        // route it and keep the flow running. `route_err` logs the cause.
        let rendered = self.render(bytes).route_err(&FAILURE, LogLevel::Warn)?;

        debug!(
            logger,
            "Parsed {} observation(s) into {} byte(s) of {}",
            rendered.observation_count,
            rendered.body.len(),
            self.output_format
        );

        Ok(
            TransformedFlowFile::new(&SUCCESS, Some(rendered.body.into()))
                .with_attributes(rendered.container_attributes)
                .with_attribute(MIME_TYPE_ATTRIBUTE, self.mime_type())
                .with_attribute(
                    OBSERVATION_COUNT_ATTRIBUTE,
                    rendered.observation_count.to_string(),
                )
                .with_attribute(FORMAT_ATTRIBUTE, self.output_format.to_string()),
        )
    }
}

pub(crate) mod processor_definition;

#[cfg(test)]
mod tests;
