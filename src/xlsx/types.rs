use std::fmt;

#[derive(Debug, Clone)]
pub struct XlsxMetadata {
    pub sheets: Vec<String>,
    pub rows: usize,
    pub cols: usize,
    pub non_empty_cells: usize,
}

#[derive(Debug, Clone)]
pub enum XlsxError {
    NoBackend,
    EmptyInput,
    InvalidDpi(u32),
    PageOutOfRange { requested: usize, total: usize },
    Parse(String),
    Convert(String),
    Render(String),
    Image(String),
    SheetNotFound(String),
}

impl fmt::Display for XlsxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XlsxError::NoBackend => write!(f, "no xlsx backend enabled"),
            XlsxError::EmptyInput => write!(f, "empty xlsx input"),
            XlsxError::InvalidDpi(dpi) => write!(f, "invalid dpi: {dpi}"),
            XlsxError::PageOutOfRange { requested, total } => {
                write!(f, "page {requested} out of range ({total} pages)")
            }
            XlsxError::Parse(msg) => write!(f, "xlsx parse failed: {msg}"),
            XlsxError::Convert(msg) => write!(f, "xlsx convert failed: {msg}"),
            XlsxError::Render(msg) => write!(f, "xlsx render failed: {msg}"),
            XlsxError::Image(msg) => write!(f, "image conversion failed: {msg}"),
            XlsxError::SheetNotFound(name) => write!(f, "sheet not found: {name}"),
        }
    }
}

impl std::error::Error for XlsxError {}

pub(crate) fn check_dpi(dpi: u32) -> Result<f64, XlsxError> {
    if dpi == 0 {
        return Err(XlsxError::InvalidDpi(dpi));
    }
    Ok(dpi as f64)
}

pub(crate) fn check_input(xlsx: &[u8]) -> Result<(), XlsxError> {
    if xlsx.is_empty() {
        return Err(XlsxError::EmptyInput);
    }
    Ok(())
}
