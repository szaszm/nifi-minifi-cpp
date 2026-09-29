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

use super::{ChecksumVerification, OutputFormat};
use minifi_native::Property;

pub(crate) const OUTPUT_FORMAT: Property<OutputFormat> = Property::new(
    "Output Format",
    "How the per-observation JSON objects are framed. 'ndjson' writes one object per line, \
     splittable downstream with SplitText at a Line Split Count of 1. 'array' writes a single \
     JSON array, splittable with SplitJson. Both carry identical objects.",
)
.with_default(OutputFormat::Ndjson.into_str());

pub(crate) const CHECKSUM_VERIFICATION: Property<ChecksumVerification> = Property::new(
    "Checksum Verification",
    "Whether to verify each record's Adler-32 checksum. PQDIF has no 'checksum absent' flag and \
     many writers leave the field zero, so 'whenPresent' verifies the records that carry one and \
     accepts those that do not. Use 'always' for untrusted input, at the cost of rejecting files \
     from writers that never compute it.",
)
.with_default(ChecksumVerification::WhenPresent.into_str());

pub(crate) const INCLUDE_SAMPLE_VALUES: Property<bool> = Property::new(
    "Include Sample Values",
    "If true, each series carries its decoded samples under 'values'. Set to false to emit only \
     the channel/series structure and each series' 'sample_count' — a single observation can hold \
     hundreds of thousands of samples, and the metadata is often all a flow needs.",
)
.with_default("true");
