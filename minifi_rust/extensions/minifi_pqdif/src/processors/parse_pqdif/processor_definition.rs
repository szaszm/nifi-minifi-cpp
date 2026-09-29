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

use super::*;
use minifi_native::{
    OutputAttribute, ProcessorDefinition, ProcessorInputRequirement, PropertyDefinition,
    Relationship, property_definitions,
};

impl ProcessorDefinition for ParsePqdif {
    const DESCRIPTION: &'static str = "Parses a PQDIF (IEEE Std 1159.3) power-quality recording and writes its observations as \
         JSON. One FlowFile in, one FlowFile out: the observations are framed either as \
         newline-delimited JSON or as a single JSON array, both of which a downstream SplitText or \
         SplitJson can fan out if per-observation FlowFiles are wanted. File-level container \
         metadata is written to 'pqdif.*' attributes rather than into the content, so it survives \
         such a split.";
    const INPUT_REQUIREMENT: ProcessorInputRequirement = ProcessorInputRequirement::Required;
    const SUPPORTS_DYNAMIC_PROPERTIES: bool = false;
    const SUPPORTS_DYNAMIC_RELATIONSHIPS: bool = false;
    const OUTPUT_ATTRIBUTES: &'static [OutputAttribute] = output_attributes::ALL;
    const RELATIONSHIPS: &'static [Relationship] = &[SUCCESS, FAILURE];

    const PROPERTIES: &'static [PropertyDefinition] = property_definitions![
        properties::OUTPUT_FORMAT,
        properties::CHECKSUM_VERIFICATION,
        properties::INCLUDE_SAMPLE_VALUES,
    ];
}
