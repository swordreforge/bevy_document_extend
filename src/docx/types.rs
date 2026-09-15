use std::fmt;

#[derive(Debug, Clone)]
pub struct DocxMetadata {
    pub pages: usize,
    pub paragraphs: usize,
    pub tables: usize,
    pub words: usize,
}

#[derive(Debug, Clone)]
pub enum DocxError {
    NoBackend,
    EmptyInput,
    InvalidDpi(u32),
    PageOutOfRange { requested: usize, total: usize },
    Parse(String),
    Convert(String),
    Render(String),
    Image(String),
}

impl fmt::Display for DocxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocxError::NoBackend => write!(f, "no docx backend enabled"),
            DocxError::EmptyInput => write!(f, "empty docx input"),
            DocxError::InvalidDpi(dpi) => write!(f, "invalid dpi: {dpi}"),
            DocxError::PageOutOfRange { requested, total } => {
                write!(f, "page {requested} out of range ({total} pages)")
            }
            DocxError::Parse(msg) => write!(f, "docx parse failed: {msg}"),
            DocxError::Convert(msg) => write!(f, "docx convert failed: {msg}"),
            DocxError::Render(msg) => write!(f, "docx render failed: {msg}"),
            DocxError::Image(msg) => write!(f, "image conversion failed: {msg}"),
        }
    }
}

impl std::error::Error for DocxError {}

pub(crate) fn check_dpi(dpi: u32) -> Result<f64, DocxError> {
    if dpi == 0 {
        return Err(DocxError::InvalidDpi(dpi));
    }
    Ok(dpi as f64)
}

pub(crate) fn check_input(docx: &[u8]) -> Result<(), DocxError> {
    if docx.is_empty() {
        return Err(DocxError::EmptyInput);
    }
    Ok(())
}
