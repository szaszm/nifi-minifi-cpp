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

//! Builders that hand-construct valid PQDIF byte arrays for integration
//! tests. Documented byte-by-byte so this doubles as a decoder-of-the-decoder.

// Each integration test binary compiles the fixtures module afresh, and
// unused-code lints fire per-binary — so a builder used only by
// `logical_smoke.rs` looks dead when compiled into `physical_smoke.rs`. Silence
// the noise at the shared entry point rather than annotating every builder.
#[allow(dead_code)]
pub mod synthetic;
