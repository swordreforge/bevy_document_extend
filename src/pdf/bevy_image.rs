use super::types::{ImageBuffer, PdfError};
use bevy::image::Image;

pub fn to_bevy_image(buffer: &ImageBuffer) -> Result<Image, PdfError> {
    crate::common::to_bevy_image(buffer).map_err(PdfError::Image)
}
