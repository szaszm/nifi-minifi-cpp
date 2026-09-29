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

//! Builders that hand-construct valid PQDIF byte arrays, for tests.
//!
//! Gated behind the off-by-default `test-fixtures` feature, so nothing here
//! reaches a normal dependent's binary. It lives in `src/` rather than
//! `tests/` so that dependents — notably the `minifi_pqdif` extension wrapping
//! this crate — can drive their own tests with genuine PQDIF bytes instead of
//! maintaining a second writer that could drift from this one.
//!
//! Enable it as a dev-dependency:
//!
//! ```toml
//! [dev-dependencies]
//! pqdif = { path = "...", features = ["test-fixtures"] }
//! ```

// Not every builder is used by every dependent; that is expected of a fixture
// library and is not worth an annotation per builder.
#[allow(dead_code)]
pub mod synthetic;
