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

//! Physical (record + element) layer of the PQDIF format.
//!
//! A PQDIF file is a singly-linked list of records. Each record has a
//! 64-byte header (see [`RecordHeader`]) that carries its two GUIDs, sizes,
//! Adler-32 checksum, and the absolute file offset of the *next* record.
//! Following the header is the record body, whose contents are laid out as
//! a tree of typed [`Element`]s (Scalar / Vector / Collection). The body may
//! be compressed with per-record `zlib`-wrapped deflate; the container
//! record is always uncompressed and carries the compression settings.
//!
//! Callers usually want the higher-level [`crate::logical`] layer instead,
//! which builds typed records out of the element tree.

pub mod checksum;
pub mod compression;
pub mod element;
pub mod header;
pub mod parser;
pub mod physical_type;

pub use checksum::ChecksumMode;
pub use compression::{CompressionAlgorithm, CompressionStyle};
pub use element::{CollectionElement, Element, ScalarElement, VectorElement};
pub use header::RecordHeader;
pub use parser::{PhysicalParser, PhysicalRecord};
pub use physical_type::PhysicalType;
