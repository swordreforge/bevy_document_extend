#![cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]

use bevy_document_extend::{PdfError, available_backends, probe, rasterize_page};

#[test]
fn backends_advertised() {
    let backends = available_backends();
    assert!(!backends.is_empty());
    #[cfg(feature = "pdf-hayro")]
    assert!(backends.contains(&"hayro"));
    #[cfg(feature = "pdf-zpdf")]
    assert!(backends.contains(&"zpdf"));
}

#[test]
fn empty_input_rejected() {
    assert!(matches!(probe(&[]), Err(PdfError::EmptyInput)));
    assert!(matches!(
        rasterize_page(&[], 0, 150),
        Err(PdfError::EmptyInput)
    ));
}

#[test]
fn invalid_dpi_rejected_before_parse() {
    assert!(matches!(
        rasterize_page(b"not a pdf", 0, 0),
        Err(PdfError::InvalidDpi(0))
    ));
}

#[test]
fn garbage_bytes_surface_parse_error() {
    assert!(matches!(
        rasterize_page(b"not a pdf", 0, 150),
        Err(PdfError::Parse(_) | PdfError::PageOutOfRange { .. })
    ));
}
