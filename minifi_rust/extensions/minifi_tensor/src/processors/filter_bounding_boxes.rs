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

use crate::processors::filter_bounding_boxes::processor_definition::{
    BACKGROUND_CLASS_INDEX, BOX_FORMAT, BOX_OUTPUT_INDEX, CONFIDENCE_THRESHOLD, FAILURE,
    IOU_THRESHOLD, OUTPUT_ATTRIBUTE_NAME, SCORE_ACTIVATION, SCORE_OUTPUT_INDEX, SUCCESS,
};
use crate::utils::bounding_box::BoundingBox;
use minifi_native::error;
use minifi_native::macros::{ComponentIdentifier, PropertyType};
use minifi_native::{
    FlowFileTransform, GetAttribute, GetId, GetProperty, InputStream, Logger, MinifiError,
    Schedule, TransformedFlowFile, debug, unwrap_or_route,
};
use std::collections::HashMap;
use strum_macros::{Display, EnumString, IntoStaticStr, VariantNames};

/// Wire format the model uses to encode a single box's four floats.
#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "PascalCase", const_into_str)]
pub(crate) enum BoxFormat {
    /// `[x_min, y_min, x_max, y_max]` — SSD, MobileNet-SSD, most PyTorch models.
    Xyxy,
    /// `[y_min, x_min, y_max, x_max]` — TensorFlow Object Detection API.
    Yxyx,
    /// `[cx, cy, w, h]` — YOLOv3/5/8 raw output (center + size).
    Cxcywh,
}

/// Activation applied to the raw per-class score vector before argmax.
#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "PascalCase", const_into_str)]
pub(crate) enum ScoreActivation {
    /// Standard cross-class softmax. Assumes classes are mutually exclusive
    /// (SSD / MobileNet-SSD raw logits).
    Softmax,
    /// Per-class sigmoid. Classes are independent (YOLOv5/v8 style).
    Sigmoid,
    /// No activation — argmax over raw scores; max score used as confidence.
    /// Use when the model already emits probabilities.
    None,
}

/// Convert the four floats at `box_floats[offset..offset+4]` into a canonical
/// `(x_min, y_min, x_max, y_max)` tuple, regardless of the source layout.
fn decode_box(box_floats: &[f32], offset: usize, format: BoxFormat) -> (f32, f32, f32, f32) {
    let a = box_floats[offset];
    let b = box_floats[offset + 1];
    let c = box_floats[offset + 2];
    let d = box_floats[offset + 3];
    match format {
        BoxFormat::Xyxy => (a, b, c, d),
        BoxFormat::Yxyx => (b, a, d, c),
        BoxFormat::Cxcywh => {
            let (cx, cy, w, h) = (a, b, c, d);
            (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
        }
    }
}

/// Result of scoring one box: winning class id + its confidence in [0, 1] for
/// Softmax/Sigmoid, or the raw score for `None`.
struct ScoredClass {
    class_id: usize,
    confidence: f32,
}

/// Pick the winning class for one box's per-class scores, applying the chosen
/// activation and honouring the background-class filter.
fn score_box(
    logits: &[f32],
    activation: ScoreActivation,
    background_class_index: Option<usize>,
) -> ScoredClass {
    let should_skip = |class_id: usize, num_classes: usize| -> bool {
        match background_class_index {
            Some(idx) => num_classes > 1 && class_id == idx,
            None => false,
        }
    };

    match activation {
        ScoreActivation::Softmax => {
            let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let sum_exp: f32 = logits.iter().map(|&l| (l - max_logit).exp()).sum();

            let mut best = ScoredClass {
                class_id: 0,
                confidence: 0.0,
            };
            for (class_id, &logit) in logits.iter().enumerate() {
                if should_skip(class_id, logits.len()) {
                    continue;
                }
                let prob = (logit - max_logit).exp() / sum_exp;
                if prob > best.confidence {
                    best = ScoredClass {
                        class_id,
                        confidence: prob,
                    };
                }
            }
            best
        }
        ScoreActivation::Sigmoid => {
            let mut best = ScoredClass {
                class_id: 0,
                confidence: 0.0,
            };
            for (class_id, &logit) in logits.iter().enumerate() {
                if should_skip(class_id, logits.len()) {
                    continue;
                }
                let prob = 1.0 / (1.0 + (-logit).exp());
                if prob > best.confidence {
                    best = ScoredClass {
                        class_id,
                        confidence: prob,
                    };
                }
            }
            best
        }
        ScoreActivation::None => {
            let mut best = ScoredClass {
                class_id: 0,
                confidence: f32::NEG_INFINITY,
            };
            for (class_id, &score) in logits.iter().enumerate() {
                if should_skip(class_id, logits.len()) {
                    continue;
                }
                if score > best.confidence {
                    best = ScoredClass {
                        class_id,
                        confidence: score,
                    };
                }
            }
            best
        }
    }
}

#[derive(ComponentIdentifier)]
pub(crate) struct FilterBoundingBoxes {
    confidence_threshold: f32,
    iou_threshold: f32,
    score_output_index: usize,
    box_output_index: usize,
    box_format: BoxFormat,
    score_activation: ScoreActivation,
    /// `None` disables background suppression.
    background_class_index: Option<usize>,
}

impl Schedule for FilterBoundingBoxes {
    fn schedule<Ctx: GetProperty, L: Logger>(
        context: &Ctx,
        _logger: &L,
    ) -> Result<Self, MinifiError> {
        let confidence_threshold = context.get_req_property::<f32>(&CONFIDENCE_THRESHOLD)?;
        let iou_threshold = context.get_req_property::<f32>(&IOU_THRESHOLD)?;
        let score_output_index = context.get_req_property::<usize>(&SCORE_OUTPUT_INDEX)?;
        let box_output_index = context.get_req_property::<usize>(&BOX_OUTPUT_INDEX)?;
        let box_format = context.get_req_property::<BoxFormat>(&BOX_FORMAT)?;
        let score_activation = context.get_req_property::<ScoreActivation>(&SCORE_ACTIVATION)?;
        // `-1` (or any negative) turns off background suppression. `>=0`
        // selects the class id treated as "no object".
        let background_raw = context.get_req_property::<i64>(&BACKGROUND_CLASS_INDEX)?;
        let background_class_index = if background_raw < 0 {
            None
        } else {
            Some(background_raw as usize)
        };

        Ok(Self {
            confidence_threshold,
            iou_threshold,
            score_output_index,
            box_output_index,
            box_format,
            score_activation,
            background_class_index,
        })
    }
}

impl FilterBoundingBoxes {
    /// Read the byte length of tract output `i` from the attributes.
    fn read_output_bytes<Ctx: GetAttribute>(context: &Ctx, i: usize) -> Result<usize, MinifiError> {
        let attr_name = format!("tract.output.{}.bytes", i);
        let raw = context
            .get_attribute(&attr_name)?
            .ok_or_else(|| MinifiError::trigger_err(format!("Missing {} attribute", attr_name)))?;
        raw.parse::<usize>()
            .map_err(|e| MinifiError::trigger_err(format!("Invalid {}: {}", attr_name, e)))
    }

    /// Slice `raw` sequentially per model output index. Given the concatenated
    /// payload emitted by InvokeTractModel, extract the bytes for output
    /// `target_index` using the per-output byte-length attributes 0..=max_idx.
    fn slice_output<'a, Ctx: GetAttribute>(
        context: &Ctx,
        raw: &'a [u8],
        target_index: usize,
    ) -> Result<&'a [u8], MinifiError> {
        let mut cursor = 0usize;
        // Walk from output 0 to target_index, advancing `cursor` past each
        // preceding tensor's bytes.
        for i in 0..=target_index {
            let len = Self::read_output_bytes(context, i)?;
            if i == target_index {
                let end = cursor + len;
                if end > raw.len() {
                    return Err(MinifiError::trigger_err(format!(
                        "Payload smaller than declared output {} slice",
                        target_index
                    )));
                }
                return Ok(&raw[cursor..end]);
            }
            cursor += len;
        }
        unreachable!()
    }

    fn bytes_to_f32s(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn get_original_hw<Context: GetAttribute>(
        context: &Context,
    ) -> Result<(f32, f32), MinifiError> {
        let orig_w = context
            .get_required_attribute("image.original.width")?
            .parse::<f32>()?;

        let orig_h = context
            .get_required_attribute("image.original.height")?
            .parse::<f32>()?;

        Ok((orig_h, orig_w))
    }

    fn get_target_hw<Context: GetAttribute>(context: &Context) -> Result<(f32, f32), MinifiError> {
        let shape_attr = context.get_required_attribute("tensor.shape")?;
        let shape_parts: Vec<&str> = shape_attr.split(',').collect();
        let target_h: f32 = shape_parts[2].parse()?;
        let target_w: f32 = shape_parts[3].parse()?;
        Ok((target_h, target_w))
    }
}

impl FlowFileTransform for FilterBoundingBoxes {
    fn transform<'a, Context: GetProperty + GetAttribute + GetId, LoggerImpl: Logger>(
        &self,
        context: &Context,
        input_stream: &'a mut dyn InputStream,
        logger: &LoggerImpl,
    ) -> Result<TransformedFlowFile<'a>, MinifiError> {
        let mut raw_bytes = Vec::new();
        input_stream.read_to_end(&mut raw_bytes)?;

        let score_bytes = unwrap_or_route!(
            Self::slice_output(context, &raw_bytes, self.score_output_index),
            &FAILURE,
            logger,
            "slice score output"
        );
        let box_bytes = unwrap_or_route!(
            Self::slice_output(context, &raw_bytes, self.box_output_index),
            &FAILURE,
            logger,
            "slice box output"
        );

        let (orig_h, orig_w) = unwrap_or_route!(Self::get_original_hw(context), &FAILURE, logger);
        let (target_h, target_w) = unwrap_or_route!(Self::get_target_hw(context), &FAILURE, logger);

        let scale = (target_w / orig_w).min(target_h / orig_h);
        let pad_x = (target_w - (orig_w * scale)) / 2.0;
        let pad_y = (target_h - (orig_h * scale)) / 2.0;

        let score_floats = Self::bytes_to_f32s(score_bytes);
        let box_floats = Self::bytes_to_f32s(box_bytes);

        if box_floats.len() % 4 != 0 {
            return Err(MinifiError::trigger_err(
                "Box tensor byte length is not a multiple of 16 (4 f32 per box)",
            ));
        }
        let num_boxes = box_floats.len() / 4;
        if num_boxes == 0 {
            debug!(logger, "No boxes to filter; emitting empty array");
            let mut attributes = HashMap::new();
            attributes.insert("object.count".to_string(), "0".to_string());
            attributes.insert("mime.type".to_string(), "application/json".to_string());
            return Ok(TransformedFlowFile::new(
                &SUCCESS,
                Some(b"[]".to_vec()),
                attributes,
            ));
        }
        if score_floats.len() % num_boxes != 0 {
            return Err(MinifiError::trigger_err(format!(
                "Scores length ({}) not divisible by number of boxes ({})",
                score_floats.len(),
                num_boxes
            )));
        }
        let num_classes = score_floats.len() / num_boxes;

        let mut valid_boxes = Vec::new();

        debug!(
            logger,
            "Filtering {} boxes across {} potential classes (activation={:?}, box_format={:?})...",
            num_boxes,
            num_classes,
            self.score_activation,
            self.box_format
        );

        for i in 0..num_boxes {
            let logits = &score_floats[i * num_classes..(i + 1) * num_classes];
            let scored = score_box(logits, self.score_activation, self.background_class_index);
            if scored.confidence >= self.confidence_threshold {
                let (raw_x_min, raw_y_min, raw_x_max, raw_y_max) =
                    decode_box(&box_floats, i * 4, self.box_format);

                let true_x_min = (((raw_x_min * target_w) - pad_x) / scale) / orig_w;
                let true_y_min = (((raw_y_min * target_h) - pad_y) / scale) / orig_h;
                let true_x_max = (((raw_x_max * target_w) - pad_x) / scale) / orig_w;
                let true_y_max = (((raw_y_max * target_h) - pad_y) / scale) / orig_h;

                valid_boxes.push(BoundingBox {
                    class_id: scored.class_id,
                    confidence: scored.confidence,
                    x_min: true_x_min.clamp(0.0, 1.0),
                    y_min: true_y_min.clamp(0.0, 1.0),
                    x_max: true_x_max.clamp(0.0, 1.0),
                    y_max: true_y_max.clamp(0.0, 1.0),
                });
            }
        }

        debug!(
            logger,
            "Found {} boxes exceeding the {} threshold.",
            valid_boxes.len(),
            self.confidence_threshold
        );

        let filtered_boxes =
            BoundingBox::apply_non_maximum_suppression(valid_boxes, self.iou_threshold);

        let json_output = unwrap_or_route!(
            serde_json::to_vec(&filtered_boxes),
            &FAILURE,
            logger,
            "serialize json"
        );

        let mut attributes = HashMap::new();
        attributes.insert("object.count".to_string(), filtered_boxes.len().to_string());
        attributes.insert("mime.type".to_string(), "application/json".to_string());

        match context.get_property::<String>(&OUTPUT_ATTRIBUTE_NAME)? {
            None => Ok(TransformedFlowFile::new(
                &SUCCESS,
                Some(json_output),
                attributes,
            )),
            Some(output_attr) => {
                attributes.insert(output_attr, serde_json::to_string(&filtered_boxes).unwrap());
                Ok(TransformedFlowFile::new(&SUCCESS, None, attributes))
            }
        }
    }
}

mod processor_definition;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_box_xyxy_is_identity() {
        let raw = [10.0, 20.0, 30.0, 40.0];
        let (x0, y0, x1, y1) = decode_box(&raw, 0, BoxFormat::Xyxy);
        assert_eq!((x0, y0, x1, y1), (10.0, 20.0, 30.0, 40.0));
    }

    #[test]
    fn test_decode_box_yxyx_swaps_axes() {
        let raw = [20.0, 10.0, 40.0, 30.0]; // (y_min, x_min, y_max, x_max)
        let (x0, y0, x1, y1) = decode_box(&raw, 0, BoxFormat::Yxyx);
        assert_eq!((x0, y0, x1, y1), (10.0, 20.0, 30.0, 40.0));
    }

    #[test]
    fn test_decode_box_cxcywh_converts_to_corners() {
        // Centre (100, 200), width 20, height 40 → corners (90,180)-(110,220)
        let raw = [100.0, 200.0, 20.0, 40.0];
        let (x0, y0, x1, y1) = decode_box(&raw, 0, BoxFormat::Cxcywh);
        assert!((x0 - 90.0).abs() < 1e-6);
        assert!((y0 - 180.0).abs() < 1e-6);
        assert!((x1 - 110.0).abs() < 1e-6);
        assert!((y1 - 220.0).abs() < 1e-6);
    }

    #[test]
    fn test_score_box_softmax_picks_argmax_excluding_background() {
        // 3 classes: bg strongly favoured, but bg is class 0 which we skip.
        let logits = [5.0, 0.5, 2.0];
        let scored = score_box(&logits, ScoreActivation::Softmax, Some(0));
        assert_eq!(scored.class_id, 2);
        assert!(scored.confidence > 0.0 && scored.confidence < 1.0);
    }

    #[test]
    fn test_score_box_sigmoid_treats_classes_independently() {
        // Sigmoid(2.0) ≈ 0.881, so class 1 wins over class 0 regardless of
        // relative magnitudes.
        let logits = [-3.0, 2.0];
        let scored = score_box(&logits, ScoreActivation::Sigmoid, None);
        assert_eq!(scored.class_id, 1);
        assert!((scored.confidence - 0.8807).abs() < 1e-3);
    }

    #[test]
    fn test_score_box_none_takes_raw_argmax() {
        let logits = [0.1, 0.7, 0.2];
        let scored = score_box(&logits, ScoreActivation::None, None);
        assert_eq!(scored.class_id, 1);
        assert!((scored.confidence - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_score_box_background_disabled_keeps_class_zero() {
        let logits = [5.0, 0.5];
        let scored = score_box(&logits, ScoreActivation::Softmax, None);
        assert_eq!(scored.class_id, 0);
    }
}
