pub mod common;

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub mod pdf;

#[cfg(feature = "docx-rdocx")]
pub mod docx;

#[cfg(any(feature = "xlsx-calamine", feature = "xlsx-office2pdf"))]
pub mod xlsx;

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub use pdf::{
    DocMetadata, ImageBuffer, PdfError, RasterBackend, available_backends, default_backend, probe,
    rasterize_page, rasterize_page_to_bevy,
};
