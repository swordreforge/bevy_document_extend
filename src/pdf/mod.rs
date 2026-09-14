pub mod backend;
pub mod bevy_image;
pub mod types;

#[cfg(feature = "pdf-hayro")]
pub mod hayro_backend;
#[cfg(feature = "pdf-zpdf")]
pub mod zpdf_backend;

pub use backend::RasterBackend;
pub use bevy_image::to_bevy_image;
pub use types::{DocMetadata, ImageBuffer, PdfError};

#[allow(clippy::vec_init_then_push)]
pub fn available_backends() -> Vec<&'static str> {
    let mut names = Vec::new();
    #[cfg(feature = "pdf-hayro")]
    names.push("hayro");
    #[cfg(feature = "pdf-zpdf")]
    names.push("zpdf");
    names
}

#[allow(unreachable_code)]
pub fn default_backend() -> Box<dyn RasterBackend> {
    #[cfg(feature = "pdf-hayro")]
    return Box::new(hayro_backend::HayroBackend);
    #[cfg(all(not(feature = "pdf-hayro"), feature = "pdf-zpdf"))]
    return Box::new(zpdf_backend::ZpdfBackend);
    panic!("no pdf backend enabled");
}

pub fn page_count(pdf: &[u8]) -> Result<usize, PdfError> {
    default_backend().page_count(pdf)
}

pub fn probe(pdf: &[u8]) -> Result<DocMetadata, PdfError> {
    default_backend().probe(pdf)
}

pub fn rasterize_page(pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PdfError> {
    default_backend().rasterize_page(pdf, page, dpi)
}

pub fn rasterize_page_to_bevy(
    pdf: &[u8],
    page: usize,
    dpi: u32,
) -> Result<bevy::image::Image, PdfError> {
    to_bevy_image(&rasterize_page(pdf, page, dpi)?)
}
