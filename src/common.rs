#[cfg(feature = "render")]
use bevy::asset::RenderAssetUsages;
#[cfg(feature = "render")]
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
        let (chunks, _) = self.rgba.as_chunks::<4>();
        for px in chunks {
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

#[cfg(feature = "render")]
pub fn to_bevy_image(buffer: &ImageBuffer) -> Result<Image, String> {
    let dynamic = buffer.to_dynamic_raw()?;
    let mut image = Image::from_dynamic(
        dynamic,
        true,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // Anisotropic filtering keeps downscaled text sharp: at fit-to-width the
    // page texture is minified (more texels than physical pixels), and the
    // default `anisotropy_clamp = 1` blurs near-horizontal/vertical glyph
    // stems. 8x is widely supported and cheap for a single page quad.
    // (No mipmaps: Bevy `Image` defaults to a single mip level, so the min
    // filter does the work — the fix is texel density + anisotropy, not mips.)
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_anisotropic_filter(8);
    image.sampler = ImageSampler::Descriptor(sampler);
    Ok(image)
}
