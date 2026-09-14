use super::types::{ImageBuffer, PdfError};
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};

pub fn to_bevy_image(buffer: &ImageBuffer) -> Result<Image, PdfError> {
    let dynamic = buffer.to_dynamic()?;
    let mut image = Image::from_dynamic(
        dynamic,
        true,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    Ok(image)
}
