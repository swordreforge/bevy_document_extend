pub mod backend;
pub mod types;

#[cfg(feature = "pptx-office")]
pub mod office_backend;

pub use backend::PptxBackend;
pub use types::{PptxError, PptxMetadata};

#[allow(clippy::vec_init_then_push)]
pub fn available_backends() -> Vec<&'static str> {
    let mut names = Vec::new();
    #[cfg(feature = "pptx-office")]
    names.push("office2pdf");
    names
}

#[allow(unreachable_code)]
pub fn default_backend() -> Box<dyn PptxBackend> {
    #[cfg(feature = "pptx-office")]
    return Box::new(office_backend::OfficePptxBackend);
    panic!("no pptx backend enabled");
}

pub fn probe(pptx: &[u8]) -> Result<PptxMetadata, PptxError> {
    default_backend().probe(pptx)
}

pub fn slide_count(pptx: &[u8]) -> Result<usize, PptxError> {
    default_backend().slide_count(pptx)
}

pub fn extract_text(pptx: &[u8]) -> Result<String, PptxError> {
    default_backend().extract_text(pptx)
}

pub fn to_pdf(pptx: &[u8]) -> Result<Vec<u8>, PptxError> {
    default_backend().to_pdf(pptx)
}

pub fn page_count(pptx: &[u8]) -> Result<usize, PptxError> {
    default_backend().page_count(pptx)
}

pub fn rasterize_page(
    pptx: &[u8],
    page: usize,
    dpi: u32,
) -> Result<crate::common::ImageBuffer, PptxError> {
    default_backend().rasterize_page(pptx, page, dpi)
}

pub fn rasterize_page_to_bevy(
    pptx: &[u8],
    page: usize,
    dpi: u32,
) -> Result<bevy::image::Image, PptxError> {
    let buffer = rasterize_page(pptx, page, dpi)?;
    crate::common::to_bevy_image(&buffer).map_err(PptxError::Image)
}

pub fn rasterize_all(pptx: &[u8], dpi: u32) -> Result<Vec<crate::common::ImageBuffer>, PptxError> {
    default_backend().rasterize_all(pptx, dpi)
}

pub fn rasterize_all_to_bevy(pptx: &[u8], dpi: u32) -> Result<Vec<bevy::image::Image>, PptxError> {
    rasterize_all(pptx, dpi)?
        .iter()
        .map(crate::common::to_bevy_image)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PptxError::Image)
}
