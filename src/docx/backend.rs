use super::types::{DocxError, DocxMetadata};
use crate::common::ImageBuffer;

pub trait DocxBackend: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn probe(&self, docx: &[u8]) -> Result<DocxMetadata, DocxError>;
    fn page_count(&self, docx: &[u8]) -> Result<usize, DocxError>;
    fn rasterize_page(&self, docx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, DocxError>;
    fn extract_text(&self, docx: &[u8]) -> Result<String, DocxError>;
}
