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

use crate::processors::classify_output::ClassifyOutput;
use crate::processors::draw_bounding_box::DrawBoundingBox;
use crate::processors::filter_bounding_boxes::FilterBoundingBoxes;
use crate::processors::image_to_tensor::ImageToTensor;
use crate::processors::invoke_tract_model::InvokeTractModel;
use crate::services::tract_model_service::TractModelService;
use minifi_native::{FlowFileTransformProcessorType, MultiThreaded};

mod processors;
mod services;
mod utils;

minifi_native::declare_minifi_extension!(
    group_name: "my.group.tnsr",
    processors: [
        (FlowFileTransformProcessorType, MultiThreaded, ImageToTensor),
        (FlowFileTransformProcessorType, MultiThreaded, InvokeTractModel),
        (FlowFileTransformProcessorType, MultiThreaded, FilterBoundingBoxes),
        (FlowFileTransformProcessorType, MultiThreaded, ClassifyOutput),
        (FlowFileTransformProcessorType, MultiThreaded, DrawBoundingBox),
    ],
    controllers: [
        TractModelService
    ]
);
