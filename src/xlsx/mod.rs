pub mod backend;
pub mod types;

#[cfg(feature = "xlsx-calamine")]
pub mod calamine_backend;

pub use backend::XlsxBackend;
pub use types::{XlsxError, XlsxMetadata};

#[allow(clippy::vec_init_then_push)]
pub fn available_backends() -> Vec<&'static str> {
    let mut names = Vec::new();
    #[cfg(feature = "xlsx-calamine")]
    names.push("calamine-office2pdf");
    names
}

#[allow(unreachable_code)]
pub fn default_backend() -> Box<dyn XlsxBackend> {
    #[cfg(feature = "xlsx-calamine")]
    return Box::new(calamine_backend::CalamineOfficeBackend);
    panic!("no xlsx backend enabled");
}

pub fn probe(xlsx: &[u8]) -> Result<XlsxMetadata, XlsxError> {
    default_backend().probe(xlsx)
}

pub fn sheet_names(xlsx: &[u8]) -> Result<Vec<String>, XlsxError> {
    default_backend().sheet_names(xlsx)
}

pub fn extract_text(xlsx: &[u8], sheet: Option<&str>) -> Result<String, XlsxError> {
    default_backend().extract_text(xlsx, sheet)
}

pub fn to_pdf(xlsx: &[u8]) -> Result<Vec<u8>, XlsxError> {
    default_backend().to_pdf(xlsx)
}

pub fn page_count(xlsx: &[u8]) -> Result<usize, XlsxError> {
    default_backend().page_count(xlsx)
}

pub fn rasterize_page(
    xlsx: &[u8],
    page: usize,
    dpi: u32,
) -> Result<crate::common::ImageBuffer, XlsxError> {
    default_backend().rasterize_page(xlsx, page, dpi)
}

pub fn rasterize_page_to_bevy(
    xlsx: &[u8],
    page: usize,
    dpi: u32,
) -> Result<bevy::image::Image, XlsxError> {
    let buffer = rasterize_page(xlsx, page, dpi)?;
    crate::common::to_bevy_image(&buffer).map_err(XlsxError::Image)
}

pub fn rasterize_all(xlsx: &[u8], dpi: u32) -> Result<Vec<crate::common::ImageBuffer>, XlsxError> {
    default_backend().rasterize_all(xlsx, dpi)
}

pub fn rasterize_all_to_bevy(xlsx: &[u8], dpi: u32) -> Result<Vec<bevy::image::Image>, XlsxError> {
    rasterize_all(xlsx, dpi)?
        .iter()
        .map(crate::common::to_bevy_image)
        .collect::<Result<Vec<_>, _>>()
        .map_err(XlsxError::Image)
}
