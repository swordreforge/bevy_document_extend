pub mod common;
pub mod viewer;

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub mod pdf;

#[cfg(feature = "docx-rdocx")]
pub mod docx;

#[cfg(feature = "xlsx-calamine")]
pub mod xlsx;

#[cfg(feature = "docx-rdocx")]
mod docx_viewer;
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
mod pdf_viewer;
#[cfg(feature = "xlsx-calamine")]
mod xlsx_viewer;

#[cfg(feature = "docx-rdocx")]
pub use docx_viewer::view_docx;
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub use pdf::{
    DocMetadata, ImageBuffer, PdfError, RasterBackend, available_backends, default_backend, probe,
    rasterize_page, rasterize_page_to_bevy,
};
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub use pdf_viewer::view_pdf;
#[cfg(feature = "xlsx-calamine")]
pub use xlsx_viewer::view_xlsx;

pub use viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
