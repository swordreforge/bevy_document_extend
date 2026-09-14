use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};

#[derive(Debug, Clone)]
pub struct ImageBuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ImageBuffer {
    /// Composite over an opaque paper-white background.
    ///
    /// Renderers (hayro/zpdf) emit premultiplied-by-nothing RGBA where
    /// untouched page area is transparent black `(0,0,0,0)`. Viewers expect
    /// paper, so flatten against white. Fully opaque pixels pass through
    /// untouched.
    pub fn flattened_on_white(&self) -> Self {
        let mut rgba = Vec::with_capacity(self.rgba.len());
        for px in self.rgba.chunks_exact(4) {
            let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
            if a == 255 {
                rgba.extend_from_slice(px);
            } else {
                rgba.push(((r * a + 255 * (255 - a)) / 255) as u8);
                rgba.push(((g * a + 255 * (255 - a)) / 255) as u8);
                rgba.push(((b * a + 255 * (255 - a)) / 255) as u8);
                rgba.push(255);
            }
        }
        Self {
            width: self.width,
            height: self.height,
            rgba,
        }
    }

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
