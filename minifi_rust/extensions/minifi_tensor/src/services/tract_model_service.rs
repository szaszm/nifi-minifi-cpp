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

use crate::services::tract_model_service::service_definition::{MODEL_FILE_PATH, MODEL_FORMAT};
use minifi_native::macros::{ComponentIdentifier, PropertyType};
use minifi_native::{EnableControllerService, GetProperty, Logger, MinifiError, debug, info};
use strum_macros::{Display, EnumString, IntoStaticStr, VariantNames};
use tract::prelude::*;

mod service_definition;

#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "PascalCase", const_into_str)]
pub(crate) enum ModelFormat {
    Auto,
    Onnx,
    Nnef,
}

/// Resolved format after any auto-detection.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResolvedFormat {
    Onnx,
    Nnef,
}

impl ModelFormat {
    fn resolve(self, path: &str) -> Result<ResolvedFormat, MinifiError> {
        match self {
            ModelFormat::Onnx => Ok(ResolvedFormat::Onnx),
            ModelFormat::Nnef => Ok(ResolvedFormat::Nnef),
            ModelFormat::Auto => {
                let lower = path.to_ascii_lowercase();
                if lower.ends_with(".onnx") {
                    Ok(ResolvedFormat::Onnx)
                } else if lower.ends_with(".nnef")
                    || lower.ends_with(".nnef.tgz")
                    || lower.ends_with(".nnef.tar")
                    || lower.ends_with(".nnef.tar.gz")
                    || std::path::Path::new(path).is_dir()
                {
                    Ok(ResolvedFormat::Nnef)
                } else {
                    Err(MinifiError::controller_service_err(format!(
                        "Could not auto-detect model format from '{}'. Set 'Model format' to \
                         'Onnx' or 'Nnef' explicitly.",
                        path
                    )))
                }
            }
        }
    }
}

#[derive(ComponentIdentifier)]
pub(crate) struct TractModelService {
    runnable_model: Runnable,
}

impl EnableControllerService for TractModelService {
    fn enable<Ctx: GetProperty, L: Logger>(context: &Ctx, logger: &L) -> Result<Self, MinifiError>
    where
        Self: Sized,
    {
        let model_path = context.get_req_property::<String>(&MODEL_FILE_PATH)?;
        let format = context.get_req_property::<ModelFormat>(&MODEL_FORMAT)?;
        let resolved = format.resolve(&model_path)?;

        info!(
            logger,
            "Loading Tract model ({:?}) from: {}", resolved, model_path
        );

        let model = match resolved {
            ResolvedFormat::Onnx => tract::onnx()
                .map_err(|e| {
                    MinifiError::controller_service_err(format!(
                        "Failed to init ONNX parser: {}",
                        e
                    ))
                })?
                .load(&model_path)
                .map_err(|e| {
                    MinifiError::controller_service_err(format!("Failed to load ONNX file: {}", e))
                })?
                .into_model()
                .map_err(|e| {
                    MinifiError::controller_service_err(format!(
                        "Failed to parse ONNX model: {}",
                        e
                    ))
                })?,
            ResolvedFormat::Nnef => tract::nnef()
                .map_err(|e| {
                    MinifiError::controller_service_err(format!(
                        "Failed to init NNEF parser: {}",
                        e
                    ))
                })?
                .load(&model_path)
                .map_err(|e| {
                    MinifiError::controller_service_err(format!("Failed to load NNEF model: {}", e))
                })?,
        };

        debug!(
            logger,
            "Optimizing and compiling the model for the host CPU..."
        );

        let runtime = tract::runtime_for_name("default").map_err(|e| {
            MinifiError::controller_service_err(format!("Failed to init Tract runtime: {}", e))
        })?;

        let runnable_model = runtime.prepare(model).map_err(|e| {
            MinifiError::controller_service_err(format!("Failed to prepare runnable model: {}", e))
        })?;

        info!(logger, "Successfully loaded and compiled Tract model.");

        Ok(Self { runnable_model })
    }
}

impl TractModelService {
    pub fn run_inference(
        &self,
        inputs: impl IntoIterator<Item = Tensor>,
    ) -> Result<Vec<Tensor>, MinifiError> {
        let vec_inputs: Vec<Tensor> = inputs.into_iter().collect();

        self.runnable_model
            .run(vec_inputs)
            .map_err(|e| MinifiError::controller_service_err(format!("Inference failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_explicit_formats_bypass_detection() {
        assert_eq!(
            ModelFormat::Onnx.resolve("/tmp/no-extension").unwrap(),
            ResolvedFormat::Onnx
        );
        assert_eq!(
            ModelFormat::Nnef.resolve("/tmp/no-extension").unwrap(),
            ResolvedFormat::Nnef
        );
    }

    #[test]
    fn test_resolve_auto_by_onnx_extension() {
        assert_eq!(
            ModelFormat::Auto.resolve("/models/face.onnx").unwrap(),
            ResolvedFormat::Onnx
        );
        assert_eq!(
            ModelFormat::Auto.resolve("/MODELS/FACE.ONNX").unwrap(),
            ResolvedFormat::Onnx
        );
    }

    #[test]
    fn test_resolve_auto_by_nnef_extension() {
        assert_eq!(
            ModelFormat::Auto
                .resolve("/models/mobilenet.nnef.tgz")
                .unwrap(),
            ResolvedFormat::Nnef
        );
        assert_eq!(
            ModelFormat::Auto.resolve("/models/mobilenet.nnef").unwrap(),
            ResolvedFormat::Nnef
        );
    }

    #[test]
    fn test_resolve_auto_errors_on_unknown() {
        assert!(ModelFormat::Auto.resolve("/tmp/no-hint").is_err());
    }
}
