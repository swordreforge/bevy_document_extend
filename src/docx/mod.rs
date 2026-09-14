pub mod backend;
pub mod types;

#[cfg(feature = "docx-rdocx")]
pub mod rdocx_backend;

pub use backend::DocxBackend;
pub use types::{DocxError, DocxMetadata};

#[allow(clippy::vec_init_then_push)]
pub fn available_backends() -> Vec<&'static str> {
    let mut names = Vec::new();
    #[cfg(feature = "docx-rdocx")]
    names.push("rdocx");
    names
}

#[allow(unreachable_code)]
pub fn default_backend() -> Box<dyn DocxBackend> {
    #[cfg(feature = "docx-rdocx")]
    return Box::new(rdocx_backend::RdocxBackend);
    panic!("no docx backend enabled");
}

pub fn probe(docx: &[u8]) -> Result<DocxMetadata, DocxError> {
    default_backend().probe(docx)
}

pub fn page_count(docx: &[u8]) -> Result<usize, DocxError> {
    default_backend().page_count(docx)
}

pub fn rasterize_page(
    docx: &[u8],
    page: usize,
    dpi: u32,
) -> Result<crate::common::ImageBuffer, DocxError> {
    default_backend().rasterize_page(docx, page, dpi)
}

pub fn rasterize_page_to_bevy(
    docx: &[u8],
    page: usize,
    dpi: u32,
) -> Result<bevy::image::Image, DocxError> {
    let buffer = rasterize_page(docx, page, dpi)?;
    crate::common::to_bevy_image(&buffer).map_err(DocxError::Image)
}

pub fn extract_text(docx: &[u8]) -> Result<String, DocxError> {
    default_backend().extract_text(docx)
}
