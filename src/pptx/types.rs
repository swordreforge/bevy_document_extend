use std::fmt;

#[derive(Debug, Clone)]
pub struct PptxMetadata {
    pub slides: usize,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PptxError {
    NoBackend,
    EmptyInput,
    InvalidDpi(u32),
    PageOutOfRange { requested: usize, total: usize },
    Parse(String),
    Convert(String),
    Render(String),
    Image(String),
}

impl fmt::Display for PptxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PptxError::NoBackend => write!(f, "no pptx backend enabled"),
            PptxError::EmptyInput => write!(f, "empty pptx input"),
            PptxError::InvalidDpi(dpi) => write!(f, "invalid dpi: {dpi}"),
            PptxError::PageOutOfRange { requested, total } => {
                write!(f, "page {requested} out of range ({total} pages)")
            }
            PptxError::Parse(msg) => write!(f, "pptx parse failed: {msg}"),
            PptxError::Convert(msg) => write!(f, "pptx convert failed: {msg}"),
            PptxError::Render(msg) => write!(f, "pptx render failed: {msg}"),
            PptxError::Image(msg) => write!(f, "image conversion failed: {msg}"),
        }
    }
}

impl std::error::Error for PptxError {}

pub(crate) fn check_dpi(dpi: u32) -> Result<f64, PptxError> {
    if dpi == 0 {
        return Err(PptxError::InvalidDpi(dpi));
    }
    Ok(dpi as f64)
}

pub(crate) fn check_input(pptx: &[u8]) -> Result<(), PptxError> {
    if pptx.is_empty() {
        return Err(PptxError::EmptyInput);
    }
    Ok(())
}
