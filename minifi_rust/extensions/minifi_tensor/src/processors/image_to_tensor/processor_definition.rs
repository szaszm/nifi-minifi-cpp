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

use crate::processors::image_to_tensor::{
    ColorFormat, ImageToTensor, ResizeFilter, ResizeMode, TensorShapeFormat,
};
use minifi_native::PropertyConstraints::{AllowedValues, NoConstraints, Validator};
use minifi_native::{
    OutputAttribute, ProcessorDefinition, ProcessorInputRequirement, Property, Relationship,
    StandardPropertyValidator,
};
use strum::VariantNames;

pub(super) const TARGET_WIDTH: Property = Property {
    name: "Target width",
    description: "Width in pixels of the tensor written to the output flow file. The decoded image \
                  is resized to this width before normalisation.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: None,
    constraints: Validator(StandardPropertyValidator::U64Validator),
};

pub(super) const TARGET_HEIGHT: Property = Property {
    name: "Target height",
    description: "Height in pixels of the tensor written to the output flow file. The decoded image \
                  is resized to this height before normalisation.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: None,
    constraints: Validator(StandardPropertyValidator::U64Validator),
};

pub(super) const RESIZE_FILTER: Property = Property {
    name: "Resize filter",
    description: "Interpolation filter applied when resizing the decoded image. Nearest is fastest \
                  but blocky; Bilinear is a good default; Bicubic and Lanczos3 are higher-quality \
                  but slower.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some(ResizeFilter::Bilinear.into_str()),
    constraints: AllowedValues(ResizeFilter::VARIANTS),
};

pub(super) const RESIZE_MODE: Property = Property {
    name: "Resize mode",
    description: "How the source image is fitted into the target dimensions. 'Stretch' scales each \
                  axis independently, distorting aspect ratio. 'Letterbox' preserves aspect ratio \
                  and pads the remaining border with 'Letterbox pad value' (applied in normalised \
                  output space).",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some(ResizeMode::Stretch.into_str()),
    constraints: AllowedValues(ResizeMode::VARIANTS),
};

pub(super) const LETTERBOX_PAD_VALUE: Property = Property {
    name: "Letterbox pad value",
    description: "Value written for padding pixels when 'Resize mode' is 'Letterbox'. This is a \
                  normalised value (post mean/std), so 0.0 corresponds to a neutral input for most \
                  networks. Ignored when 'Resize mode' is 'Stretch'.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some("0.0"),
    constraints: NoConstraints,
};

pub(super) const COLOR_FORMAT: Property = Property {
    name: "Color format",
    description: "Colour space of the output tensor. RGB and BGR produce three-channel tensors \
                  (channel order determined by the format); Grayscale produces a single-channel \
                  luma tensor.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some(ColorFormat::Rgb.into_str()),
    constraints: AllowedValues(ColorFormat::VARIANTS),
};

pub(super) const TENSOR_SHAPE_FORMAT: Property = Property {
    name: "Tensor shape format",
    description: "Memory layout of the output tensor. CHW (channels-first) is typical for PyTorch/\
                  ONNX detectors. HWC (channels-last) matches TensorFlow/TFLite. Ignored for \
                  Grayscale (always effectively 1xHxW). The 'tensor.shape' attribute is written to \
                  match the chosen layout.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some(TensorShapeFormat::Chw.into_str()),
    constraints: AllowedValues(TensorShapeFormat::VARIANTS),
};

pub(super) const MEAN: Property = Property {
    name: "Mean",
    description: "Mean subtracted from each pixel before dividing by 'Standard Deviation'. Accepts \
                  either a single value (broadcast to all channels) or three comma-separated \
                  values applied per channel in the order dictated by 'Color format'. Example: \
                  '0.485, 0.456, 0.406' for ImageNet-style RGB normalisation.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some("0.0"),
    constraints: NoConstraints,
};

pub(super) const STD_DEV: Property = Property {
    name: "Standard Deviation",
    description: "Divisor applied after subtracting 'Mean'. Accepts a single value (broadcast) or \
                  three comma-separated values (per channel). Must be non-zero. Example: '255.0' \
                  to scale u8 pixels into [0.0, 1.0]; '0.229, 0.224, 0.225' for ImageNet.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some("255.0"),
    constraints: NoConstraints,
};

pub(super) const PIXEL_DIVISOR: Property = Property {
    name: "Pixel divisor",
    description: "Divisor applied to raw u8 pixel values before subtracting 'Mean' and dividing \
                  by 'Standard Deviation'. Defaults to 1.0 (mean/std interpreted in [0, 255] pixel \
                  space, e.g. UltraFace's mean=127, std=128). Set to 255 to bring pixels into \
                  [0.0, 1.0] first so ImageNet-style mean/std values like '0.485, 0.456, 0.406' / \
                  '0.229, 0.224, 0.225' can be used directly, matching the PyTorch / torchvision / \
                  ONNX MobileNet convention. Must be non-zero.",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: false,
    default_value: Some("1.0"),
    constraints: NoConstraints,
};

pub(super) const SUCCESS: Relationship = Relationship {
    name: "success",
    description: "The input image was decoded and converted to a tensor.",
};

pub(super) const FAILURE: Relationship = Relationship {
    name: "failure",
    description: "The input flow file could not be decoded as an image.",
};

pub(super) const TENSOR_SHAPE_ATTR: OutputAttribute = OutputAttribute {
    name: "tensor.shape",
    relationships: &["success"],
    description: "Comma-separated dimensions of the output tensor in the chosen layout, always \
                  including a leading batch dimension of 1 (e.g. '1,3,224,224' for RGB CHW).",
};

pub(super) const TENSOR_DTYPE_ATTR: OutputAttribute = OutputAttribute {
    name: "tensor.dtype",
    relationships: &["success"],
    description: "Element type of the values in the output tensor. Currently always 'f32'.",
};

pub(super) const IMAGE_ORIGINAL_HEIGHT_ATTR: OutputAttribute = OutputAttribute {
    name: "image.original.height",
    relationships: &["success"],
    description: "The height of the original image before the resizing.",
};

pub(super) const IMAGE_ORIGINAL_WIDTH_ATTR: OutputAttribute = OutputAttribute {
    name: "image.original.width",
    relationships: &["success"],
    description: "The width of the original image before the resizing.",
};

impl ProcessorDefinition for ImageToTensor {
    const DESCRIPTION: &'static str = "Decodes an image from the flow file content and converts it into a normalised numeric \
         tensor suitable for feeding into a downstream inference processor such as \
         InvokeTractModel. Supports RGB / BGR / Grayscale, CHW / HWC layouts, stretch or \
         letterbox resizing, and scalar or per-channel mean/std normalisation. The output payload \
         is the raw little-endian f32 tensor; the 'tensor.shape' and 'tensor.dtype' attributes \
         describe its layout.";
    const INPUT_REQUIREMENT: ProcessorInputRequirement = ProcessorInputRequirement::Required;
    const SUPPORTS_DYNAMIC_PROPERTIES: bool = false;
    const SUPPORTS_DYNAMIC_RELATIONSHIPS: bool = false;
    const OUTPUT_ATTRIBUTES: &'static [OutputAttribute] = &[TENSOR_SHAPE_ATTR, TENSOR_DTYPE_ATTR];
    const RELATIONSHIPS: &'static [Relationship] = &[SUCCESS, FAILURE];
    const PROPERTIES: &'static [Property] = &[
        TARGET_WIDTH,
        TARGET_HEIGHT,
        RESIZE_FILTER,
        RESIZE_MODE,
        LETTERBOX_PAD_VALUE,
        COLOR_FORMAT,
        TENSOR_SHAPE_FORMAT,
        MEAN,
        STD_DEV,
        PIXEL_DIVISOR,
    ];
}
