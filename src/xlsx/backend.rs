use super::types::{XlsxError, XlsxMetadata};
use crate::common::ImageBuffer;

pub trait XlsxBackend: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn probe(&self, xlsx: &[u8]) -> Result<XlsxMetadata, XlsxError>;
    fn sheet_names(&self, xlsx: &[u8]) -> Result<Vec<String>, XlsxError>;
    fn extract_text(&self, xlsx: &[u8], sheet: Option<&str>) -> Result<String, XlsxError>;
    fn to_pdf(&self, xlsx: &[u8]) -> Result<Vec<u8>, XlsxError>;
    fn page_count(&self, xlsx: &[u8]) -> Result<usize, XlsxError>;
    fn rasterize_page(&self, xlsx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, XlsxError>;
    fn rasterize_all(&self, xlsx: &[u8], dpi: u32) -> Result<Vec<ImageBuffer>, XlsxError>;
}
