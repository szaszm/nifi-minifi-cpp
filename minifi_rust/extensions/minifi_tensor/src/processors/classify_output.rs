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

use crate::processors::classify_output::processor_definition::{
    CONFIDENCE_THRESHOLD, FAILURE, LABEL_INDEX_OFFSET, LABELS_FILE_PATH, SCORE_ACTIVATION,
    SCORE_OUTPUT_INDEX, SUCCESS, TOP_K,
};
use minifi_native::error;
use minifi_native::macros::{ComponentIdentifier, PropertyType};
use minifi_native::{
    FlowFileTransform, GetAttribute, GetId, GetProperty, InputStream, Logger, MinifiError,
    Schedule, TransformedFlowFile, debug, unwrap_or_route,
};
use serde::Serialize;
use std::collections::HashMap;
use strum_macros::{Display, EnumString, IntoStaticStr, VariantNames};

mod processor_definition;

/// Activation applied to the raw score vector before ranking. Kept parallel to
/// the one in `filter_bounding_boxes` — this processor doesn't need argmax
/// short-circuiting, so we recompute probabilities for every class.
#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "PascalCase", const_into_str)]
pub(crate) enum ScoreActivation {
    /// Cross-class softmax; classes are mutually exclusive (typical for
    /// ImageNet-trained ResNet/MobileNet/EfficientNet ONNX exports).
    Softmax,
    /// Per-class sigmoid; classes are independent (multi-label classifiers).
    Sigmoid,
    /// Pass-through — the model already emits probabilities.
    None,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
struct Prediction {
    class_id: usize,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_name: Option<String>,
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    // Degenerate input (all-NaN, or all -inf → max is -inf) — fall back to
    // uniform. Skipping this would produce NaNs downstream: exp(-inf - -inf)
    // is exp(NaN) which is NaN, so the normal path can't recover.
    if !max_logit.is_finite() {
        return vec![1.0 / logits.len() as f32; logits.len()];
    }
    let exps: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
    let sum: f32 = exps.iter().sum();
    if sum == 0.0 || !sum.is_finite() {
        return vec![1.0 / logits.len() as f32; logits.len()];
    }
    exps.iter().map(|e| e / sum).collect()
}

fn sigmoid_vec(logits: &[f32]) -> Vec<f32> {
    logits.iter().map(|&l| 1.0 / (1.0 + (-l).exp())).collect()
}

fn apply_activation(scores: &[f32], activation: ScoreActivation) -> Vec<f32> {
    match activation {
        ScoreActivation::Softmax => softmax(scores),
        ScoreActivation::Sigmoid => sigmoid_vec(scores),
        ScoreActivation::None => scores.to_vec(),
    }
}

/// Load a newline-separated labels file at schedule time. Trailing whitespace
/// is stripped; blank lines are preserved as empty strings so line-number ↔
/// class-id alignment stays exact.
fn load_labels(path: &str) -> Result<Vec<String>, MinifiError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        MinifiError::trigger_err(format!("Failed to read labels file '{}': {}", path, e))
    })?;
    Ok(content
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect())
}

/// Return the indices of the top `k` entries in `scores`, descending. Ties are
/// broken by lower index first (stable partial sort). `k` is clamped to
/// `scores.len()`.
fn top_k_indices(scores: &[f32], k: usize) -> Vec<usize> {
    let k = k.min(scores.len());
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    // Partial sort: after this, the first k elements are the top-k in order.
    idx.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    idx.truncate(k);
    idx
}

#[derive(ComponentIdentifier)]
pub(crate) struct ClassifyOutput {
    top_k: usize,
    score_output_index: usize,
    score_activation: ScoreActivation,
    confidence_threshold: f32,
    /// Loaded once at schedule time. Empty vec if no path was configured.
    labels: Vec<String>,
    /// Added to the model's class id when indexing into `labels`. Non-zero for
    /// label files whose first line is a dummy/background entry (e.g. the
    /// standard `imagenet_slim_labels.txt` with class 0 = "dummy").
    label_index_offset: usize,
}

impl Schedule for ClassifyOutput {
    fn schedule<Ctx: GetProperty, L: Logger>(
        context: &Ctx,
        _logger: &L,
    ) -> Result<Self, MinifiError>
    where
        Self: Sized,
    {
        let top_k = context.get_req_property::<usize>(&TOP_K)?;
        if top_k == 0 {
            return Err(MinifiError::trigger_err("Top K must be >= 1"));
        }
        let score_output_index = context.get_req_property::<usize>(&SCORE_OUTPUT_INDEX)?;
        let score_activation = context.get_req_property::<ScoreActivation>(&SCORE_ACTIVATION)?;
        let confidence_threshold = context.get_req_property::<f32>(&CONFIDENCE_THRESHOLD)?;
        // Labels file is optional — load and cache when configured; empty vec
        // means "emit class_id only".
        let labels = match context.get_property::<String>(&LABELS_FILE_PATH)? {
            Some(path) if !path.is_empty() => load_labels(&path)?,
            _ => Vec::new(),
        };
        let label_index_offset = context.get_req_property::<usize>(&LABEL_INDEX_OFFSET)?;

        Ok(Self {
            top_k,
            score_output_index,
            score_activation,
            confidence_threshold,
            labels,
            label_index_offset,
        })
    }
}

impl ClassifyOutput {
    /// Slice bytes for output tensor `target_index` out of the concatenated
    /// InvokeTractModel payload, walking per-output byte-length attributes.
    fn slice_output<'a, Ctx: GetAttribute>(
        context: &Ctx,
        raw: &'a [u8],
        target_index: usize,
    ) -> Result<&'a [u8], MinifiError> {
        let mut cursor = 0usize;
        for i in 0..=target_index {
            let attr_name = format!("tract.output.{}.bytes", i);
            let raw_attr = context.get_attribute(&attr_name)?.ok_or_else(|| {
                MinifiError::trigger_err(format!("Missing {} attribute", attr_name))
            })?;
            let len: usize = raw_attr
                .parse()
                .map_err(|e| MinifiError::trigger_err(format!("Invalid {}: {}", attr_name, e)))?;
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

    fn bytes_to_f32s(bytes: &[u8]) -> Result<Vec<f32>, MinifiError> {
        if !bytes.len().is_multiple_of(4) {
            return Err(MinifiError::trigger_err(
                "Score tensor byte length is not a multiple of 4 (f32)",
            ));
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    fn label_for(&self, class_id: usize) -> Option<String> {
        self.labels
            .get(class_id.checked_add(self.label_index_offset)?)
            .cloned()
    }
}

impl FlowFileTransform for ClassifyOutput {
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
        let raw_scores = unwrap_or_route!(
            Self::bytes_to_f32s(score_bytes),
            &FAILURE,
            logger,
            "parse score bytes"
        );

        if raw_scores.is_empty() {
            return Err(MinifiError::trigger_err(
                "Score tensor is empty; nothing to classify",
            ));
        }

        // Most classifiers produce a shape like [1, num_classes] — one batch,
        // one vector. We treat the whole flattened tensor as a single score
        // vector; if the model produced [batch, num_classes] with batch > 1
        // the ranking would still be over the full flat array, but there is
        // no way to disambiguate here without also reading tract.output.i.shape.
        // ImageToTensor always produces batch=1 today so this is fine in
        // practice; we document the assumption in the processor description.
        let probabilities = apply_activation(&raw_scores, self.score_activation);
        let ranked = top_k_indices(&probabilities, self.top_k);

        let predictions: Vec<Prediction> = ranked
            .into_iter()
            .filter(|&class_id| probabilities[class_id] >= self.confidence_threshold)
            .map(|class_id| Prediction {
                class_id,
                confidence: probabilities[class_id],
                class_name: self.label_for(class_id),
            })
            .collect();

        debug!(
            logger,
            "Classified {} classes → top {} kept above threshold {}",
            raw_scores.len(),
            predictions.len(),
            self.confidence_threshold
        );

        let json_output = unwrap_or_route!(
            serde_json::to_vec(&predictions),
            &FAILURE,
            logger,
            "serialize predictions"
        );

        let mut attributes = HashMap::new();
        attributes.insert("mime.type".to_string(), "application/json".to_string());
        attributes.insert("class.count".to_string(), predictions.len().to_string());
        // Cheap top-1 attributes for Expression Language routing without JSON
        // parsing. Absent when nothing cleared the threshold.
        if let Some(top) = predictions.first() {
            attributes.insert("class.top1.id".to_string(), top.class_id.to_string());
            attributes.insert(
                "class.top1.confidence".to_string(),
                top.confidence.to_string(),
            );
            if let Some(name) = &top.class_name {
                attributes.insert("class.top1.name".to_string(), name.clone());
            }
        }

        Ok(TransformedFlowFile::new(
            &SUCCESS,
            Some(json_output),
            attributes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minifi_native::{MockLogger, MockProcessContext};
    use std::io::Cursor;

    fn make_processor(top_k: usize, activation: ScoreActivation) -> ClassifyOutput {
        ClassifyOutput {
            top_k,
            score_output_index: 0,
            score_activation: activation,
            confidence_threshold: 0.0,
            labels: Vec::new(),
            label_index_offset: 0,
        }
    }

    #[test]
    fn test_softmax_preserves_ranking() {
        let logits = [1.0, 3.0, 2.0];
        let probs = softmax(&logits);
        assert!(probs[1] > probs[2] && probs[2] > probs[0]);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_top_k_indices_descending_and_clamped() {
        let scores = [0.1, 0.9, 0.5, 0.3];
        assert_eq!(top_k_indices(&scores, 2), vec![1, 2]);
        assert_eq!(top_k_indices(&scores, 10), vec![1, 2, 3, 0]);
    }

    #[test]
    fn test_top_k_indices_tiebreak_by_lower_index() {
        let scores = [0.5, 0.5, 0.5];
        assert_eq!(top_k_indices(&scores, 3), vec![0, 1, 2]);
    }

    fn build_payload(scores: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(scores.len() * 4);
        for s in scores {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        bytes
    }

    fn context_with_scores(scores: &[f32]) -> MockProcessContext {
        let mut ctx = MockProcessContext::new();
        ctx.attributes.insert(
            "tract.output.0.bytes".to_string(),
            (scores.len() * 4).to_string(),
        );
        ctx
    }

    #[test]
    fn test_transform_returns_top_k_json() {
        let processor = make_processor(3, ScoreActivation::Softmax);
        let logits = vec![1.0f32, 4.0, 2.0, 0.5, 3.0];
        let context = context_with_scores(&logits);
        let payload = build_payload(&logits);
        let mut stream = Cursor::new(payload);
        let result = processor
            .transform(&context, &mut stream, &MockLogger::new())
            .expect("transform succeeds");

        assert_eq!(result.target_relationship(), SUCCESS.name);
        assert_eq!(
            result.attributes_to_add().get("class.top1.id").unwrap(),
            "1"
        );
        assert_eq!(result.attributes_to_add().get("class.count").unwrap(), "3");

        let json_bytes = result.into_bytes().unwrap().unwrap();
        let json = String::from_utf8(json_bytes).unwrap();
        // Expect three entries, ranked class_id 1, then 4, then 2.
        assert!(json.contains("\"class_id\":1"));
        assert!(json.contains("\"class_id\":4"));
        assert!(json.contains("\"class_id\":2"));
    }

    #[test]
    fn test_transform_omits_class_name_when_labels_absent() {
        let processor = make_processor(1, ScoreActivation::None);
        let scores = vec![0.1f32, 0.9];
        let context = context_with_scores(&scores);
        let mut stream = Cursor::new(build_payload(&scores));
        let result = processor
            .transform(&context, &mut stream, &MockLogger::new())
            .unwrap();
        // Snapshot the top1.name absence before into_bytes consumes `result`.
        let has_top1_name = result.attributes_to_add().contains_key("class.top1.name");
        let json = String::from_utf8(result.into_bytes().unwrap().unwrap()).unwrap();
        assert!(!json.contains("class_name"));
        assert!(!has_top1_name);
    }

    #[test]
    fn test_transform_looks_up_labels() {
        let mut processor = make_processor(1, ScoreActivation::None);
        processor.labels = vec![
            "tench".into(),
            "goldfish".into(),
            "great_white_shark".into(),
        ];
        let scores = vec![0.1f32, 0.9, 0.5];
        let context = context_with_scores(&scores);
        let mut stream = Cursor::new(build_payload(&scores));
        let result = processor
            .transform(&context, &mut stream, &MockLogger::new())
            .unwrap();
        assert_eq!(
            result.attributes_to_add().get("class.top1.name").unwrap(),
            "goldfish"
        );
        let json = String::from_utf8(result.into_bytes().unwrap().unwrap()).unwrap();
        assert!(json.contains("\"class_name\":\"goldfish\""));
    }

    #[test]
    fn test_label_index_offset_shifts_lookup() {
        // Mirrors the ImageNet slim labels layout: line 0 is a dummy entry, so
        // model class 0 should resolve to labels[1] = "tench".
        let mut processor = make_processor(1, ScoreActivation::None);
        processor.labels = vec![
            "dummy".into(),
            "tench".into(),
            "goldfish".into(),
            "great_white_shark".into(),
        ];
        processor.label_index_offset = 1;
        let scores = vec![0.9f32, 0.1, 0.0]; // model class 0 wins
        let context = context_with_scores(&scores);
        let mut stream = Cursor::new(build_payload(&scores));
        let result = processor
            .transform(&context, &mut stream, &MockLogger::new())
            .unwrap();
        assert_eq!(
            result.attributes_to_add().get("class.top1.name").unwrap(),
            "tench",
            "class id 0 with offset 1 should land on labels[1]"
        );
    }

    #[test]
    fn test_transform_filters_below_confidence_threshold() {
        let mut processor = make_processor(3, ScoreActivation::None);
        processor.confidence_threshold = 0.6;
        let scores = vec![0.1f32, 0.9, 0.5];
        let context = context_with_scores(&scores);
        let mut stream = Cursor::new(build_payload(&scores));
        let result = processor
            .transform(&context, &mut stream, &MockLogger::new())
            .unwrap();
        assert_eq!(
            result.attributes_to_add().get("class.count").unwrap(),
            "1",
            "only the 0.9-scored class should survive"
        );
    }

    #[test]
    fn test_transform_missing_bytes_attribute_routes_to_failure() {
        let processor = make_processor(1, ScoreActivation::Softmax);
        let context = MockProcessContext::new(); // no tract.output.0.bytes
        let mut stream = Cursor::new(vec![0u8; 4]);
        let result = processor
            .transform(&context, &mut stream, &MockLogger::new())
            .expect("routes to failure, does not error");
        assert_eq!(result.target_relationship(), FAILURE.name);
    }

    #[test]
    fn test_bytes_to_f32s_rejects_non_multiple_of_four() {
        let bad = vec![0u8, 1, 2];
        assert!(ClassifyOutput::bytes_to_f32s(&bad).is_err());
    }

    #[test]
    fn test_softmax_handles_all_neg_infinity() {
        // Not a real-world case but exercises the degenerate branch.
        let probs = softmax(&[f32::NEG_INFINITY, f32::NEG_INFINITY]);
        assert!((probs.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }
}
