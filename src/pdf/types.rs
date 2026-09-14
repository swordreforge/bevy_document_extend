use std::fmt;

#[derive(Debug, Clone)]
pub struct DocMetadata {
    pub pages: usize,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct ImageBuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum PdfError {
    NoBackend,
    EmptyInput,
    InvalidDpi(u32),
    PageOutOfRange { requested: usize, total: usize },
    Parse(String),
    Render(String),
    Image(String),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::NoBackend => write!(f, "no pdf backend enabled"),
            PdfError::EmptyInput => write!(f, "empty pdf input"),
            PdfError::InvalidDpi(dpi) => write!(f, "invalid dpi: {dpi}"),
            PdfError::PageOutOfRange { requested, total } => {
                write!(f, "page {requested} out of range ({total} pages)")
            }
            PdfError::Parse(msg) => write!(f, "pdf parse failed: {msg}"),
            PdfError::Render(msg) => write!(f, "pdf render failed: {msg}"),
            PdfError::Image(msg) => write!(f, "image conversion failed: {msg}"),
        }
    }
}

impl std::error::Error for PdfError {}

impl ImageBuffer {
    pub fn to_dynamic(&self) -> Result<image::DynamicImage, PdfError> {
        let expected = self.width as usize * self.height as usize * 4;
        if self.rgba.len() != expected {
            return Err(PdfError::Image(format!(
                "rgba length {} does not match {}x{}",
                self.rgba.len(),
                self.width,
                self.height
            )));
        }
        let img = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or_else(|| PdfError::Image("invalid rgba dimensions".to_string()))?;
        Ok(image::DynamicImage::ImageRgba8(img))
    }
}

pub(crate) fn dpi_to_scale(dpi: u32) -> Result<f32, PdfError> {
    if dpi == 0 {
        return Err(PdfError::InvalidDpi(dpi));
    }
    Ok(dpi as f32 / 72.0)
}

pub(crate) fn check_page(page: usize, total: usize) -> Result<(), PdfError> {
    if page >= total {
        return Err(PdfError::PageOutOfRange {
            requested: page,
            total,
        });
    }
    Ok(())
}

pub(crate) fn check_input(pdf: &[u8]) -> Result<(), PdfError> {
    if pdf.is_empty() {
        return Err(PdfError::EmptyInput);
    }
    Ok(())
}
