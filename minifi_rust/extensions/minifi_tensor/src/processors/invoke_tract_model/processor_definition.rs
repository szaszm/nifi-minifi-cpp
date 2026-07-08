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

use crate::services::tract_model_service::TractModelService;
use minifi_native::PropertyConstraints::AllowedType;
use minifi_native::{
    ComponentIdentifier, OutputAttribute, ProcessorDefinition, ProcessorInputRequirement, Property,
    Relationship,
};

pub(crate) const TRACT_MODEL_SERVICE: Property = Property {
    name: "Tract model service",
    description: "Reference to a TractModelService controller service. The referenced service \
                  owns the compiled model (ONNX or NNEF) that will be evaluated for each \
                  incoming flow file.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: None,
    constraints: AllowedType(TractModelService::CLASS_NAME),
};

pub(super) const SUCCESS: Relationship = Relationship {
    name: "success",
    description: "Inference completed. The flow file's content is the concatenation of every \
                  output tensor's raw bytes in model output order.",
};

pub(super) const FAILURE: Relationship = Relationship {
    name: "failure",
    description: "The input tensor could not be built (missing/invalid tensor.shape, unsupported \
                  tensor.dtype, malformed payload) or the model failed to run.",
};

const OUTPUT_COUNT_ATTR: OutputAttribute = OutputAttribute {
    name: "tract.output.count",
    relationships: &["success"],
    description: "Number of output tensors produced by the model. Downstream processors can loop \
                  from 0 up to this count when reading per-output attributes.",
};

const OUTPUT_SHAPE_ATTR: OutputAttribute = OutputAttribute {
    name: "tract.output.{i}.shape",
    relationships: &["success"],
    description: "Comma-separated dimensions of output tensor at index i.",
};

const OUTPUT_BYTES_ATTR: OutputAttribute = OutputAttribute {
    name: "tract.output.{i}.bytes",
    relationships: &["success"],
    description: "Byte length of output tensor at index i within the concatenated payload. \
                  Consumers slice the payload sequentially using these lengths.",
};

const OUTPUT_DTYPE_ATTR: OutputAttribute = OutputAttribute {
    name: "tract.output.{i}.dtype",
    relationships: &["success"],
    description: "Element type of output tensor at index i (e.g. 'f32', 'i64'), lowercased.",
};

impl ProcessorDefinition for super::InvokeTractModel {
    const DESCRIPTION: &'static str = "Runs a single inference against the compiled model owned by the referenced \
         TractModelService. Reads the input tensor from the flow file content plus the \
         'tensor.shape' and (optionally) 'tensor.dtype' attributes produced by an upstream \
         processor such as ImageToTensor. The flow file's new content is every output tensor's \
         raw bytes concatenated in model order; per-tensor shape, byte length, and dtype are \
         written to attributes. Only a single input tensor and only f32 input dtype are \
         supported in this pass.";
    const INPUT_REQUIREMENT: ProcessorInputRequirement = ProcessorInputRequirement::Required;
    const SUPPORTS_DYNAMIC_PROPERTIES: bool = false;
    const SUPPORTS_DYNAMIC_RELATIONSHIPS: bool = false;
    const OUTPUT_ATTRIBUTES: &'static [OutputAttribute] = &[
        OUTPUT_COUNT_ATTR,
        OUTPUT_SHAPE_ATTR,
        OUTPUT_BYTES_ATTR,
        OUTPUT_DTYPE_ATTR,
    ];
    const RELATIONSHIPS: &'static [Relationship] = &[SUCCESS, FAILURE];
    const PROPERTIES: &'static [Property] = &[TRACT_MODEL_SERVICE];
}
