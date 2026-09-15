pub mod common;

#[cfg(feature = "viewer")]
pub mod viewer;

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub mod pdf;

#[cfg(feature = "docx-office")]
pub mod docx;

#[cfg(feature = "xlsx-calamine")]
pub mod xlsx;

#[cfg(feature = "pptx-office")]
pub mod pptx;

#[cfg(all(feature = "viewer", feature = "docx-office"))]
mod docx_viewer;
#[cfg(all(feature = "viewer", any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
mod pdf_viewer;
#[cfg(all(feature = "viewer", feature = "pptx-office"))]
mod pptx_viewer;
#[cfg(all(feature = "viewer", feature = "xlsx-calamine"))]
mod xlsx_viewer;

#[cfg(all(feature = "viewer", feature = "docx-office"))]
pub use docx_viewer::{view_docx, view_docx_with};
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
pub use pdf::{
    DocMetadata, ImageBuffer, PdfError, RasterBackend, available_backends, default_backend, probe,
    rasterize_page, rasterize_page_to_bevy,
};
#[cfg(all(feature = "viewer", any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
pub use pdf_viewer::{view_pdf, view_pdf_with};
#[cfg(all(feature = "viewer", feature = "pptx-office"))]
pub use pptx_viewer::{view_pptx, view_pptx_with};
#[cfg(all(feature = "viewer", feature = "xlsx-calamine"))]
pub use xlsx_viewer::{view_xlsx, view_xlsx_with};

#[cfg(feature = "viewer")]
pub use viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs, ViewerOptions};
