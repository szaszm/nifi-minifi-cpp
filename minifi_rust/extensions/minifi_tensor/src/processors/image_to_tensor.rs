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
pub(super) mod processor_definition;

use crate::processors::image_to_tensor::processor_definition::{
    COLOR_FORMAT, FAILURE, IMAGE_ORIGINAL_HEIGHT_ATTR, IMAGE_ORIGINAL_WIDTH_ATTR,
    LETTERBOX_PAD_VALUE, MEAN, PIXEL_DIVISOR, RESIZE_FILTER, RESIZE_MODE, STD_DEV, SUCCESS,
    TARGET_HEIGHT, TARGET_WIDTH, TENSOR_DTYPE_ATTR, TENSOR_SHAPE_ATTR, TENSOR_SHAPE_FORMAT,
};
use minifi_native::macros::{ComponentIdentifier, PropertyType};
use minifi_native::{
    FlowFileTransform, GetAttribute, GetControllerService, GetId, GetProperty, InputStream, Logger,
    MinifiError, Schedule, TransformedFlowFile, error, unwrap_or_route,
};
use std::collections::HashMap;
use strum_macros::{Display, EnumString, IntoStaticStr, VariantNames};

#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "PascalCase", const_into_str)]
pub(crate) enum ResizeFilter {
    /// Nearest-neighbor interpolation.
    /// The fastest algorithm, simply picking the closest pixel.
    /// Results in blocky, pixelated images, but perfect for pixel art
    /// or when performance is the absolute highest priority.
    Nearest,

    /// Bilinear interpolation.
    /// Calculates the weighted average of the 4 nearest pixels.
    /// A fast, general-purpose filter that produces smoother results
    /// than Nearest, though it can sometimes look a bit blurry.
    Bilinear,

    /// Bicubic interpolation.
    /// Considers the 16 nearest pixels to fit a smooth curve.
    /// Offers a great balance between performance and quality, producing
    /// sharper and smoother images than Bilinear. The standard for most uses.
    Bicubic,

    /// Lanczos interpolation (window size 3).
    /// Uses a mathematical function over a 6x6 pixel grid (36 pixels).
    /// Provides the highest quality and preserves sharp details best,
    /// especially when downscaling, but is the most computationally expensive.
    Lanczos3,
}

impl From<ResizeFilter> for image::imageops::FilterType {
    fn from(filter: ResizeFilter) -> Self {
        match filter {
            ResizeFilter::Nearest => image::imageops::FilterType::Nearest,
            ResizeFilter::Bilinear => image::imageops::FilterType::Triangle,
            ResizeFilter::Bicubic => image::imageops::FilterType::CatmullRom,
            ResizeFilter::Lanczos3 => image::imageops::FilterType::Lanczos3,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "UPPERCASE", const_into_str)]
pub(crate) enum ColorFormat {
    Rgb,
    Bgr,
    Grayscale,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "UPPERCASE", const_into_str)]
pub(crate) enum TensorShapeFormat {
    // Center, Height, Width
    Chw,
    // Height, Width, Center
    Hwc,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Display, EnumString, VariantNames, IntoStaticStr, PropertyType,
)]
#[strum(serialize_all = "PascalCase", const_into_str)]
pub(crate) enum ResizeMode {
    /// Resize to exactly target_width x target_height, ignoring aspect ratio.
    Stretch,
    /// Preserve aspect ratio, scale to fit inside target, and pad the remainder
    /// with `letterbox_pad_value` (applied in normalized-output space).
    Letterbox,
}

/// Parses a scalar or comma-separated triple into a per-channel Vec.
/// Returns Err if the string is neither a single float nor exactly three floats,
/// or if any component fails to parse.
fn parse_per_channel_f32(input: &str) -> Result<Vec<f32>, String> {
    let parts: Vec<&str> = input.split(',').map(|s| s.trim()).collect();
    match parts.len() {
        1 | 3 => parts
            .iter()
            .map(|p| {
                p.parse::<f32>()
                    .map_err(|e| format!("invalid float '{}': {}", p, e))
            })
            .collect(),
        n => Err(format!("expected 1 or 3 comma-separated values, got {}", n)),
    }
}

/// Returns `values[c]` when the vector has `channels` entries (per-channel),
/// or `values[0]` when it has a single entry (broadcast).
fn per_channel(values: &[f32], c: usize) -> f32 {
    if values.len() == 1 {
        values[0]
    } else {
        values[c]
    }
}

#[derive(ComponentIdentifier)]
pub(crate) struct ImageToTensor {
    target_width: u32,
    target_height: u32,
    resize_filter: ResizeFilter,
    resize_mode: ResizeMode,
    color_format: ColorFormat,
    tensor_shape_format: TensorShapeFormat,
    mean: Vec<f32>,
    std_dev: Vec<f32>,
    pixel_divisor: f32,
    letterbox_pad_value: f32,
}

impl Schedule for ImageToTensor {
    fn schedule<Ctx: GetProperty, L: Logger>(
        context: &Ctx,
        _logger: &L,
    ) -> Result<Self, MinifiError>
    where
        Self: Sized,
    {
        let target_width: u32 = context.get_req_property::<u32>(&TARGET_WIDTH)?;
        let target_height: u32 = context.get_req_property::<u32>(&TARGET_HEIGHT)?;
        let resize_filter = context.get_req_property::<ResizeFilter>(&RESIZE_FILTER)?;
        let resize_mode = context.get_req_property::<ResizeMode>(&RESIZE_MODE)?;
        let color_format = context.get_req_property::<ColorFormat>(&COLOR_FORMAT)?;
        let tensor_shape_format =
            context.get_req_property::<TensorShapeFormat>(&TENSOR_SHAPE_FORMAT)?;
        let mean_str = context.get_req_property::<String>(&MEAN)?;
        let mean = parse_per_channel_f32(&mean_str).map_err(MinifiError::trigger_err)?;
        let std_dev_str = context.get_req_property::<String>(&STD_DEV)?;
        let std_dev = parse_per_channel_f32(&std_dev_str).map_err(MinifiError::trigger_err)?;
        if std_dev.contains(&0.0) {
            return Err(MinifiError::trigger_err(
                "Standard Deviation components must be non-zero",
            ));
        }
        let pixel_divisor = context.get_req_property::<f32>(&PIXEL_DIVISOR)?;
        if pixel_divisor == 0.0 {
            return Err(MinifiError::trigger_err("Pixel divisor must be non-zero"));
        }
        let letterbox_pad_value = context.get_req_property::<f32>(&LETTERBOX_PAD_VALUE)?;

        Ok(Self {
            target_width,
            target_height,
            resize_filter,
            resize_mode,
            color_format,
            tensor_shape_format,
            mean,
            std_dev,
            pixel_divisor,
            letterbox_pad_value,
        })
    }
}

impl ImageToTensor {
    /// Resize `img` into a (target_width, target_height) RGB image according to
    /// the configured resize mode. For `Letterbox`, the returned image already
    /// has the padding baked in as pixel color (0..255); the normalized pad
    /// value is applied separately during tensor writing.
    ///
    /// Returns the resized RGB image plus a mask of "valid" pixels
    /// (`true` = came from the source, `false` = padding). The caller uses the
    /// mask to substitute `letterbox_pad_value` into normalized output.
    fn resize_rgb(&self, img: &image::DynamicImage) -> (image::RgbImage, Vec<bool>) {
        let filter: image::imageops::FilterType = self.resize_filter.into();
        match self.resize_mode {
            ResizeMode::Stretch => {
                let resized = img
                    .resize_exact(self.target_width, self.target_height, filter)
                    .to_rgb8();
                // no padding so every pixel is part of the source
                let mask = vec![true; (self.target_width * self.target_height) as usize];
                (resized, mask)
            }
            ResizeMode::Letterbox => {
                let (src_w, src_h) = (img.width() as f32, img.height() as f32);
                let scale =
                    (self.target_width as f32 / src_w).min(self.target_height as f32 / src_h);
                let new_w = (src_w * scale).round().max(1.0) as u32;
                let new_h = (src_h * scale).round().max(1.0) as u32;
                let scaled = img.resize_exact(new_w, new_h, filter).to_rgb8();

                let pad_x = (self.target_width - new_w) / 2;
                let pad_y = (self.target_height - new_h) / 2;

                // Canvas with target dimensions
                let mut canvas = image::RgbImage::from_pixel(
                    self.target_width,
                    self.target_height,
                    image::Rgb([0, 0, 0]),
                );
                // Overlay the image padded
                image::imageops::overlay(&mut canvas, &scaled, pad_x as i64, pad_y as i64);

                // Mask to track which pixel is part of source and which is padding
                let mut mask = vec![false; (self.target_width * self.target_height) as usize];
                for y in pad_y..(new_h + pad_y) {
                    for x in pad_x..(new_w + pad_x) {
                        mask[(y * self.target_width + x) as usize] = true;
                    }
                }
                (canvas, mask)
            }
        }
    }
}

impl FlowFileTransform for ImageToTensor {
    fn transform<
        'a,
        Context: GetProperty + GetControllerService + GetAttribute + GetId,
        LoggerImpl: Logger,
    >(
        &self,
        _context: &Context,
        input_stream: &'a mut dyn InputStream,
        logger: &LoggerImpl,
    ) -> Result<TransformedFlowFile<'a>, MinifiError> {
        let mut raw_bytes = Vec::new();
        input_stream.read_to_end(&mut raw_bytes)?;

        let img = unwrap_or_route!(
            image::load_from_memory(&raw_bytes),
            &FAILURE,
            logger,
            "decode image"
        );

        let num_channels: usize = match self.color_format {
            ColorFormat::Grayscale => 1,
            _ => 3,
        };

        let total_pixels = (self.target_width * self.target_height) as usize;
        let mut tensor_bytes = Vec::with_capacity(total_pixels * num_channels * 4);

        if self.color_format == ColorFormat::Grayscale {
            let (rgb_img, mask) = self.resize_rgb(&img);
            let mean = self.mean[0];
            let std_dev = self.std_dev[0];
            for (idx, pixel) in rgb_img.pixels().enumerate() {
                let val = if mask[idx] {
                    // Rec. 601 luma coefficients — matches image::to_luma8().
                    let luma =
                        0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
                    (luma / self.pixel_divisor - mean) / std_dev
                } else {
                    self.letterbox_pad_value
                };
                tensor_bytes.extend_from_slice(&val.to_le_bytes());
            }
        } else {
            let (rgb_img, mask) = self.resize_rgb(&img);
            let channel_order: [usize; 3] = match self.color_format {
                ColorFormat::Rgb => [0, 1, 2],
                ColorFormat::Bgr => [2, 1, 0],
                _ => unreachable!(),
            };

            let normalized = |px_idx: usize, out_c: usize, src_c: usize| -> f32 {
                if !mask[px_idx] {
                    return self.letterbox_pad_value;
                }
                let raw = rgb_img.get_pixel(
                    (px_idx as u32) % self.target_width,
                    (px_idx as u32) / self.target_width,
                )[src_c] as f32;
                (raw / self.pixel_divisor - per_channel(&self.mean, out_c))
                    / per_channel(&self.std_dev, out_c)
            };

            match self.tensor_shape_format {
                TensorShapeFormat::Chw => {
                    for (out_c, &src_c) in channel_order.iter().enumerate() {
                        for px_idx in 0..total_pixels {
                            let v = normalized(px_idx, out_c, src_c);
                            tensor_bytes.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                }
                TensorShapeFormat::Hwc => {
                    for px_idx in 0..total_pixels {
                        for (out_c, &src_c) in channel_order.iter().enumerate() {
                            let v = normalized(px_idx, out_c, src_c);
                            tensor_bytes.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                }
            }
        }

        let shape_str = match (self.color_format, self.tensor_shape_format) {
            (ColorFormat::Grayscale, _) => {
                format!("1,1,{},{}", self.target_height, self.target_width)
            }
            (_, TensorShapeFormat::Chw) => {
                format!(
                    "1,{},{},{}",
                    num_channels, self.target_height, self.target_width
                )
            }
            (_, TensorShapeFormat::Hwc) => {
                format!(
                    "1,{},{},{}",
                    self.target_height, self.target_width, num_channels
                )
            }
        };

        let mut attributes = HashMap::new();
        attributes.insert(TENSOR_SHAPE_ATTR.name.to_owned(), shape_str);
        attributes.insert(TENSOR_DTYPE_ATTR.name.to_owned(), "f32".to_string());
        attributes.insert(
            IMAGE_ORIGINAL_HEIGHT_ATTR.name.to_owned(),
            img.height().to_string(),
        );
        attributes.insert(
            IMAGE_ORIGINAL_WIDTH_ATTR.name.to_owned(),
            img.width().to_string(),
        );

        Ok(TransformedFlowFile::new(
            &SUCCESS,
            Some(tensor_bytes),
            attributes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{FAILURE, SUCCESS};
    use image::{ImageFormat, RgbImage};
    use minifi_native::{MockLogger, MockProcessContext};
    use std::io::Cursor;

    fn create_test_image_bytes() -> Vec<u8> {
        let mut img = RgbImage::new(2, 2);
        // Set one pixel to pure white (255, 255, 255) -> should scale to 1.0
        img.put_pixel(0, 0, image::Rgb([255, 255, 255]));
        // Set one pixel to pure black (0, 0, 0) -> should scale to 0.0
        img.put_pixel(1, 1, image::Rgb([0, 0, 0]));

        let mut bytes: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        img.write_to(&mut cursor, ImageFormat::Png).unwrap();
        bytes
    }

    fn default_processor() -> ImageToTensor {
        ImageToTensor {
            target_width: 2,
            target_height: 2,
            resize_filter: ResizeFilter::Nearest,
            resize_mode: ResizeMode::Stretch,
            color_format: ColorFormat::Rgb,
            tensor_shape_format: TensorShapeFormat::Chw,
            mean: vec![0.0],
            std_dev: vec![255.0],
            pixel_divisor: 1.0,
            letterbox_pad_value: 0.0,
        }
    }

    fn payload_as_f32(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    #[test]
    fn test_successful_rgb_chw_transform() {
        let processor = default_processor();

        let context = MockProcessContext::new();
        let input_bytes = create_test_image_bytes();
        let mut input_stream = Cursor::new(input_bytes);

        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .expect("Transform should succeed");

        assert_eq!(result.target_relationship(), SUCCESS.name);

        assert_eq!(
            result
                .attributes_to_add()
                .get("tensor.shape")
                .expect("Missing shape attribute"),
            "1,3,2,2"
        );
        assert_eq!(
            result
                .attributes_to_add()
                .get("tensor.dtype")
                .expect("Missing dtype attribute"),
            "f32"
        );

        let payload: Vec<u8> = result
            .into_bytes()
            .unwrap()
            .expect("there should be a payload");

        assert_eq!(payload.len(), 48);

        let first_f32 = payload_as_f32(&payload)[0];
        assert_eq!(first_f32, 1.0);
    }

    #[test]
    fn test_successful_grayscale_transform() {
        let processor = ImageToTensor {
            color_format: ColorFormat::Grayscale,
            ..default_processor()
        };

        let context = MockProcessContext::new();
        let input_bytes = create_test_image_bytes();
        let mut input_stream = Cursor::new(input_bytes);

        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .expect("Transform should succeed");

        assert_eq!(result.target_relationship(), SUCCESS.name);
        assert_eq!(
            result
                .attributes_to_add()
                .get("tensor.shape")
                .expect("Missing shape attribute"),
            "1,1,2,2"
        );

        let payload = result
            .into_bytes()
            .unwrap()
            .expect("there should be a payload");
        // 2x2 image * 1 channel * 4 bytes per f32 = 16 bytes
        assert_eq!(payload.len(), 16);
    }

    #[test]
    fn test_invalid_image_routes_to_failure() {
        let processor = ImageToTensor {
            target_width: 224,
            target_height: 224,
            resize_filter: ResizeFilter::Bilinear,
            ..default_processor()
        };

        let context = MockProcessContext::new();

        let invalid_bytes = vec![0x00, 0x01, 0x02, 0x03, 0x04];
        let mut input_stream = Cursor::new(invalid_bytes);

        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .expect("Transform should not panic/error, but gracefully route to FAILURE");

        assert_eq!(result.target_relationship(), FAILURE.name);
    }

    #[test]
    fn test_hwc_shape_attribute_matches_layout() {
        let processor = ImageToTensor {
            tensor_shape_format: TensorShapeFormat::Hwc,
            ..default_processor()
        };
        let context = MockProcessContext::new();
        let mut input_stream = Cursor::new(create_test_image_bytes());
        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .unwrap();
        assert_eq!(
            result.attributes_to_add().get("tensor.shape").unwrap(),
            "1,2,2,3"
        );
    }

    #[test]
    fn test_per_channel_mean_and_std() {
        // Use per-channel mean/std where R uses (mean=0, std=255) → 1.0,
        // G uses (mean=255, std=255) → 0.0, B uses (mean=0, std=127.5) → 2.0.
        let processor = ImageToTensor {
            mean: vec![0.0, 255.0, 0.0],
            std_dev: vec![255.0, 255.0, 127.5],
            ..default_processor()
        };
        let context = MockProcessContext::new();
        let mut input_stream = Cursor::new(create_test_image_bytes());
        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .unwrap();
        let payload = payload_as_f32(&result.into_bytes().unwrap().unwrap());
        // CHW layout: 4 R values, 4 G values, 4 B values. Pixel (0,0) is white.
        assert!(
            (payload[0] - 1.0).abs() < 1e-6,
            "R channel pixel(0,0) = {}",
            payload[0]
        );
        assert!(
            (payload[4] - 0.0).abs() < 1e-6,
            "G channel pixel(0,0) = {}",
            payload[4]
        );
        assert!(
            (payload[8] - 2.0).abs() < 1e-6,
            "B channel pixel(0,0) = {}",
            payload[8]
        );
    }

    #[test]
    fn test_letterbox_pads_non_square_image() {
        // Create a 4x2 image (aspect 2:1) — letterboxed into 4x4 should get
        // top/bottom padding of one row each, and the middle two rows should
        // contain the source content.
        let mut img = RgbImage::new(4, 2);
        for y in 0..2 {
            for x in 0..4 {
                img.put_pixel(x, y, image::Rgb([255, 255, 255]));
            }
        }
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let processor = ImageToTensor {
            target_width: 4,
            target_height: 4,
            resize_mode: ResizeMode::Letterbox,
            letterbox_pad_value: -1.0,
            ..default_processor()
        };
        let context = MockProcessContext::new();
        let mut input_stream = Cursor::new(bytes);
        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .unwrap();
        let payload = payload_as_f32(&result.into_bytes().unwrap().unwrap());

        // CHW, R channel first. 16 pixels per channel. Rows: 0 = pad, 1-2 = content, 3 = pad.
        // R channel:
        //   row 0 (pixels 0..4)   = pad value (-1.0)
        //   rows 1..3 (pixels 4..12) = content (1.0 after /255)
        //   row 3 (pixels 12..16) = pad value (-1.0)
        for (i, v) in payload.iter().enumerate().take(4) {
            assert!((v + 1.0).abs() < 1e-6, "expected pad at r[{}]", i);
        }
        for (i, v) in payload.iter().enumerate().take(12).skip(4) {
            assert!((v - 1.0).abs() < 1e-6, "expected content at r[{}]", i);
        }
        for (i, v) in payload.iter().enumerate().take(16).skip(12) {
            assert!((v + 1.0).abs() < 1e-6, "expected pad at r[{}]", i);
        }
    }

    #[test]
    fn test_pixel_divisor_matches_tract_imagenet_recipe() {
        // Reproduces the tract onnx-mobilenet-v2 example's normalization:
        //   (raw / 255.0 - mean_[0,1]) / std_[0,1]
        // A pure-white pixel R channel should become (1.0 - 0.485) / 0.229 ≈ 2.2489.
        let processor = ImageToTensor {
            mean: vec![0.485, 0.456, 0.406],
            std_dev: vec![0.229, 0.224, 0.225],
            pixel_divisor: 255.0,
            ..default_processor()
        };
        let context = MockProcessContext::new();
        let mut input_stream = Cursor::new(create_test_image_bytes());
        let result = processor
            .transform(&context, &mut input_stream, &MockLogger::new())
            .unwrap();
        let payload = payload_as_f32(&result.into_bytes().unwrap().unwrap());
        // Pixel (0,0) is white (255,255,255). CHW → payload[0] is R for pixel(0,0).
        let expected_r = (1.0_f32 - 0.485) / 0.229;
        assert!(
            (payload[0] - expected_r).abs() < 1e-5,
            "R channel pixel(0,0) = {}, expected {}",
            payload[0],
            expected_r
        );
    }

    #[test]
    fn test_parse_per_channel_f32_accepts_scalar_and_triple() {
        assert_eq!(parse_per_channel_f32("0.5").unwrap(), vec![0.5]);
        assert_eq!(
            parse_per_channel_f32("0.485, 0.456, 0.406").unwrap(),
            vec![0.485, 0.456, 0.406]
        );
        assert!(parse_per_channel_f32("1, 2").is_err());
        assert!(parse_per_channel_f32("1, 2, three").is_err());
    }
}
