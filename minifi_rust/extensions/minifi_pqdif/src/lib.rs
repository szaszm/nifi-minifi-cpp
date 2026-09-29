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

//! MiNiFi extension for reading PQDIF (IEEE Std 1159.3) power-quality
//! recordings.
//!
//! The parser itself is the [`pqdif`] crate in the `pqdif/` subdirectory, which
//! knows nothing about MiNiFi and is publishable on its own. This crate is the
//! adapter: it maps PQDIF's logical model onto JSON ([`json`]) and exposes that
//! as a processor ([`processors::parse_pqdif`]).

mod json;
mod processors;

use crate::processors::parse_pqdif::ParsePqdif;
use minifi_native::{FlowFileTransformProcessorType, MultiThreaded};

minifi_native::declare_minifi_extension!(
    group_name: "org.apache.nifi.minifi.rust.pqdif",
    processors: [
        (FlowFileTransformProcessorType, MultiThreaded, ParsePqdif),
    ],
    controllers: []
);
