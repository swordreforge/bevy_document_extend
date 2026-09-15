use super::types::{PptxError, PptxMetadata};
use crate::common::ImageBuffer;

pub trait PptxBackend: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn probe(&self, pptx: &[u8]) -> Result<PptxMetadata, PptxError>;
    fn slide_count(&self, pptx: &[u8]) -> Result<usize, PptxError>;
    fn extract_text(&self, pptx: &[u8]) -> Result<String, PptxError>;
    fn to_pdf(&self, pptx: &[u8]) -> Result<Vec<u8>, PptxError>;
    fn page_count(&self, pptx: &[u8]) -> Result<usize, PptxError>;
    fn rasterize_page(&self, pptx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PptxError>;
    fn rasterize_all(&self, pptx: &[u8], dpi: u32) -> Result<Vec<ImageBuffer>, PptxError>;
}
