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

use crate::processors::invoke_tract_model::processor_definition::{
    FAILURE, SUCCESS, TRACT_MODEL_SERVICE,
};
use crate::services::tract_model_service::TractModelService;
use minifi_native::error;
use minifi_native::macros::ComponentIdentifier;
use minifi_native::{
    FlowFileTransform, GetAttribute, GetControllerService, GetId, GetProperty, InputStream, Logger,
    MinifiError, Schedule, TransformedFlowFile, debug, unwrap_or_route,
};
use std::collections::HashMap;
use std::error::Error;
use tract::__ndarray_interop::TensorInterface;
use tract::Tensor;
tract::impl_ndarray_interop!();

mod processor_definition;

/// The only input dtype currently supported. Written by ImageToTensor into the
/// `tensor.dtype` attribute; anything else routes to failure rather than being
/// silently reinterpreted as f32.
const SUPPORTED_INPUT_DTYPE: &str = "f32";

#[derive(ComponentIdentifier)]
pub(crate) struct InvokeTractModel {}

impl Schedule for InvokeTractModel {
    fn schedule<Ctx: GetProperty, L: Logger>(
        _context: &Ctx,
        _logger: &L,
    ) -> Result<Self, MinifiError>
    where
        Self: Sized,
    {
        Ok(Self {})
    }
}

impl InvokeTractModel {
    fn get_payload_as_f32_array(
        input_stream: &mut dyn InputStream,
    ) -> Result<Vec<f32>, Box<dyn Error>> {
        let mut raw_bytes = Vec::new();
        input_stream.read_to_end(&mut raw_bytes)?;
        if raw_bytes.len() % 4 != 0 {
            return Err(
                MinifiError::trigger_err("Input bytes length is not a multiple of 4").into(),
            );
        }
        let mut f32_data = Vec::with_capacity(raw_bytes.len() / 4);
        for chunk in raw_bytes.chunks_exact(4) {
            let val = f32::from_le_bytes(chunk.try_into()?);
            f32_data.push(val);
        }
        Ok(f32_data)
    }

    fn get_shape<Context: GetAttribute>(context: &Context) -> Result<Vec<usize>, Box<dyn Error>> {
        let shape_str = context
            .get_attribute("tensor.shape")
            .and_then(|opt| opt.ok_or(MinifiError::trigger_err("Missing tensor.shape")))?;
        shape_str
            .split(',')
            .map(|s| s.trim().parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Returns the input dtype declared by the upstream processor via the
    /// `tensor.dtype` attribute. Missing attribute is tolerated (defaults to
    /// f32 for backwards compatibility with older flows). An unsupported value
    /// yields Err — the caller routes to `failure`.
    fn check_input_dtype<Context: GetAttribute>(context: &Context) -> Result<(), Box<dyn Error>> {
        let dtype = context.get_attribute("tensor.dtype")?;
        match dtype.as_deref() {
            None | Some(SUPPORTED_INPUT_DTYPE) => Ok(()),
            Some(other) => Err(MinifiError::trigger_err(format!(
                "Unsupported tensor.dtype '{}' (only '{}' is supported)",
                other, SUPPORTED_INPUT_DTYPE
            ))
            .into()),
        }
    }

    fn get_input_tensor<Context: GetAttribute>(
        context: &Context,
        input_stream: &mut dyn InputStream,
    ) -> Result<Tensor, Box<dyn Error>> {
        Self::check_input_dtype(context)?;
        let shape = Self::get_shape(context)?;
        let f32_data = Self::get_payload_as_f32_array(input_stream)?;

        let array = ndarray::Array::from_shape_vec(shape, f32_data)
            .map_err(|e| MinifiError::trigger_err(format!("Failed to create array: {}", e)))?;

        Ok(array.tract()?)
    }
}

impl FlowFileTransform for InvokeTractModel {
    fn transform<
        'a,
        Context: GetProperty + GetControllerService + GetAttribute + GetId,
        LoggerImpl: Logger,
    >(
        &self,
        context: &Context,
        input_stream: &'a mut dyn InputStream,
        logger: &LoggerImpl,
    ) -> Result<TransformedFlowFile<'a>, MinifiError> {
        let controller_service = context
            .get_controller_service::<TractModelService>(&TRACT_MODEL_SERVICE)?
            .ok_or(MinifiError::missing_required_property(
                "A valid usable controller service is required",
            ))?;

        let input_tensor: Tensor = unwrap_or_route!(
            Self::get_input_tensor(context, input_stream),
            &FAILURE,
            logger,
            "build input tensor"
        );

        let output_tensors = unwrap_or_route!(
            controller_service.run_inference(vec![input_tensor]),
            &FAILURE,
            logger,
            "run tract inference"
        );
        let mut output_bytes = Vec::new();
        let mut attributes = HashMap::new();
        attributes.insert(
            "tract.output.count".to_string(),
            output_tensors.len().to_string(),
        );

        for (i, tensor) in output_tensors.iter().enumerate() {
            let (datum_type, out_shape, raw_tensor_bytes) = tensor.as_bytes().map_err(|e| {
                MinifiError::trigger_err(format!("Failed to read tensor bytes: {}", e))
            })?;

            output_bytes.extend_from_slice(raw_tensor_bytes);

            let out_shape_str = out_shape
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");

            attributes.insert(format!("tract.output.{}.shape", i), out_shape_str);
            attributes.insert(
                format!("tract.output.{}.bytes", i),
                raw_tensor_bytes.len().to_string(),
            );
            // Debug-formatted DatumType renders as "F32", "I64", etc. — a
            // stable enough identifier for downstream string matching.
            attributes.insert(
                format!("tract.output.{}.dtype", i),
                format!("{:?}", datum_type).to_lowercase(),
            );

            debug!(
                logger,
                "Output {} - Shape: [{}], Dtype: {:?}, Bytes: {}",
                i,
                attributes
                    .get(&format!("tract.output.{}.shape", i))
                    .unwrap(),
                datum_type,
                raw_tensor_bytes.len()
            );
        }

        Ok(TransformedFlowFile::new(
            &SUCCESS,
            Some(output_bytes),
            attributes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minifi_native::{MockLogger, MockProcessContext};
    use std::io::Cursor;

    #[test]
    fn test_get_shape_parses_correctly() {
        let mut context = MockProcessContext::new();
        // Insert a valid shape with spaces
        context
            .attributes
            .insert("tensor.shape".to_string(), "1, 3, 224, 224".to_string());

        let shape = InvokeTractModel::get_shape(&context).expect("Should parse valid shape");
        assert_eq!(shape, vec![1, 3, 224, 224]);
    }

    #[test]
    fn test_get_shape_missing_attribute_fails() {
        let context = MockProcessContext::new(); // Empty attributes
        let result = InvokeTractModel::get_shape(&context);
        assert!(result.is_err(), "Should fail when tensor.shape is missing");
    }

    #[test]
    fn test_get_shape_invalid_data_fails() {
        let mut context = MockProcessContext::new();
        context
            .attributes
            .insert("tensor.shape".to_string(), "1, apple, 224".to_string());

        let result = InvokeTractModel::get_shape(&context);
        assert!(
            result.is_err(),
            "Should fail when shape contains non-integers"
        );
    }

    #[test]
    fn test_get_payload_as_f32_array_success() {
        let expected_floats: [f32; 3] = [1.0, 2.5, -3.5];
        let mut bytes = Vec::new();
        for f in expected_floats.iter() {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        let mut stream = Cursor::new(bytes);
        let result = InvokeTractModel::get_payload_as_f32_array(&mut stream)
            .expect("Should parse valid byte stream");
        assert_eq!(result, vec![1.0, 2.5, -3.5]);
    }

    #[test]
    fn test_get_payload_invalid_length_fails() {
        let bytes = vec![0u8, 1, 2, 3, 4];
        let mut stream = Cursor::new(bytes);

        let result = InvokeTractModel::get_payload_as_f32_array(&mut stream);
        assert!(
            result.is_err(),
            "Should error out if byte length is not a multiple of 4"
        );
    }

    #[test]
    fn test_check_input_dtype_accepts_missing_and_f32() {
        let mut context = MockProcessContext::new();
        assert!(InvokeTractModel::check_input_dtype(&context).is_ok());
        context
            .attributes
            .insert("tensor.dtype".to_string(), "f32".to_string());
        assert!(InvokeTractModel::check_input_dtype(&context).is_ok());
    }

    #[test]
    fn test_check_input_dtype_rejects_other() {
        let mut context = MockProcessContext::new();
        context
            .attributes
            .insert("tensor.dtype".to_string(), "u8".to_string());
        assert!(InvokeTractModel::check_input_dtype(&context).is_err());
    }

    #[test]
    fn test_transform_missing_controller_service_throws_error() {
        let processor = InvokeTractModel {};
        let context = MockProcessContext::new();
        let mut stream = Cursor::new(vec![]);
        let logger = MockLogger::new();

        let result = processor.transform(&context, &mut stream, &logger);

        assert!(
            result.is_err(),
            "Should throw an error when TractModelService is missing"
        );
    }
}
