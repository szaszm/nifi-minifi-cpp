use crate::utils::bounding_box::BoundingBoxes;
use image::{ImageFormat, Rgb, load_from_memory};
use minifi_native::PropertyConstraints::NoConstraints;
use minifi_native::macros::ComponentIdentifier;
use minifi_native::{
    FlowFileTransform, GetAttribute, GetControllerService, GetId, GetProperty, InputStream, Logger,
    MinifiError, OutputAttribute, ProcessorDefinition, ProcessorInputRequirement, Property,
    PropertyConstraints, PropertyType, Relationship, Schedule, StandardPropertyValidator,
    TransformedFlowFile, unwrap_or_route,
};
use std::collections::HashMap;
use std::io::Cursor;
use minifi_native::error;

pub(crate) const SUCCESS: Relationship = Relationship {
    name: "success",
    description: "Flowfiles are routed here after drawing the bounding boxes",
};

pub(crate) const FAILURE: Relationship = Relationship {
    name: "failure",
    description: "Invalid FlowFiles are routed here",
};

pub(crate) const BOUNDING_BOXES: Property = Property {
    name: "Bounding boxes",
    description: "TODO(mzink)",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: true,
    default_value: Some("${enrichment.value}"),
    constraints: NoConstraints,
};

const LINE_THICKNESS: Property = Property {
    name: "Line tickness",
    description: "TODO(mzink)",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: true,
    default_value: Some("5"),
    constraints: PropertyConstraints::Validator(StandardPropertyValidator::U64Validator),
};

const LINE_COLOR: Property = Property {
    name: "Line color",
    description: "TODO(mzink)",
    is_required: true,
    is_sensitive: false,
    supports_expr_lang: true,
    default_value: Some("[0, 255, 0]"),
    constraints: NoConstraints,
};

#[derive(Debug, ComponentIdentifier)]
pub(crate) struct DrawBoundingBox {}

impl Schedule for DrawBoundingBox {
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

struct LineColor {}
impl PropertyType for LineColor {
    type Output = Rgb<u8>;

    fn parse(s: &str) -> Result<Self::Output, MinifiError> {
        let clean_str = s.trim().trim_matches(|c| c == '[' || c == ']');
        let mut iter = clean_str.split(',');

        Ok(Rgb::<u8>([
            iter.next()
                .ok_or(MinifiError::parse_err())?
                .trim()
                .parse::<u8>()?,
            iter.next()
                .ok_or(MinifiError::parse_err())?
                .trim()
                .parse::<u8>()?,
            iter.next()
                .ok_or(MinifiError::parse_err())?
                .trim()
                .parse::<u8>()?,
        ]))
    }
}

impl FlowFileTransform for DrawBoundingBox {
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
        let line_thickness = unwrap_or_route!(
            context.get_req_property::<u32>(&LINE_THICKNESS),
            &FAILURE,
            logger,
            "getting LINE_THICKNESS"
        );
        let line_color = unwrap_or_route!(
            context.get_req_property::<LineColor>(&LINE_COLOR),
            &FAILURE,
            logger,
            "getting LINE_COLOR"
        );
        let boxes = unwrap_or_route!(
            context.get_req_property::<BoundingBoxes>(&BOUNDING_BOXES),
            &FAILURE,
            logger,
            "bounding boxes"
        );

        let mut image_bytes = Vec::new();
        input_stream.read_to_end(&mut image_bytes)?;

        let mut img = unwrap_or_route!(
            load_from_memory(&image_bytes)
                .map(|dyn_img| dyn_img.to_rgb8())
                .map_err(|_e| MinifiError::UnknownError),
            &FAILURE,
            logger
        );

        boxes
            .iter()
            .for_each(|bbox| bbox.draw_onto(&mut img, line_thickness, line_color));

        let mut output_bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut output_bytes), ImageFormat::Png)
            .map_err(|_e| MinifiError::UnknownError)?;

        Ok(TransformedFlowFile::new(
            &SUCCESS,
            Some(output_bytes),
            HashMap::new(),
        ))
    }
}

impl ProcessorDefinition for DrawBoundingBox {
    const DESCRIPTION: &'static str = "DrawBoundingBox";
    const INPUT_REQUIREMENT: ProcessorInputRequirement = ProcessorInputRequirement::Required;
    const SUPPORTS_DYNAMIC_PROPERTIES: bool = false;
    const SUPPORTS_DYNAMIC_RELATIONSHIPS: bool = false;
    const OUTPUT_ATTRIBUTES: &'static [OutputAttribute] = &[];
    const RELATIONSHIPS: &'static [Relationship] = &[SUCCESS, FAILURE];
    const PROPERTIES: &'static [Property] = &[BOUNDING_BOXES, LINE_COLOR, LINE_THICKNESS];
}

#[cfg(test)]
mod tests {
    use crate::processors::draw_bounding_box::{LINE_COLOR, LineColor};
    use minifi_native::{GetProperty, MockControllerServiceContext};

    #[test]
    fn test_parsing_colors() {
        let mock_context = MockControllerServiceContext::default();
        let default_color = mock_context
            .get_req_property::<LineColor>(&LINE_COLOR)
            .expect("we should parse this");
        let green = image::Rgb([0, 255, 0]);
        assert_eq!(default_color, green);
    }
}
