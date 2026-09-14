use super::types::{DocMetadata, ImageBuffer, PdfError};

pub trait RasterBackend: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn page_count(&self, pdf: &[u8]) -> Result<usize, PdfError>;
    fn probe(&self, pdf: &[u8]) -> Result<DocMetadata, PdfError>;
    fn rasterize_page(&self, pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PdfError>;
}
