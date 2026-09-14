use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};

#[derive(Debug, Clone)]
pub struct ImageBuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ImageBuffer {
    pub fn to_dynamic_raw(&self) -> Result<image::DynamicImage, String> {
        let expected = self.width as usize * self.height as usize * 4;
        if self.rgba.len() != expected {
            return Err(format!(
                "rgba length {} does not match {}x{}",
                self.rgba.len(),
                self.width,
                self.height
            ));
        }
        let img = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or_else(|| "invalid rgba dimensions".to_string())?;
        Ok(image::DynamicImage::ImageRgba8(img))
    }
}

pub fn to_bevy_image(buffer: &ImageBuffer) -> Result<Image, String> {
    let dynamic = buffer.to_dynamic_raw()?;
    let mut image = Image::from_dynamic(
        dynamic,
        true,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    Ok(image)
}
