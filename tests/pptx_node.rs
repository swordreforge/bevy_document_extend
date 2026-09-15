#![cfg(feature = "pptx-office")]

use bevy_document_extend::pptx::PptxError;
use bevy_document_extend::pptx::{
    available_backends, extract_text, probe, rasterize_all, rasterize_page, slide_count,
};

static PPTX: &[u8] = include_bytes!("exp/pptx/sample.pptx");

#[test]
fn backends_advertised() {
    let backends = available_backends();
    assert!(!backends.is_empty());
    #[cfg(feature = "pptx-office")]
    assert!(backends.contains(&"office2pdf"));
}

#[test]
fn empty_input_rejected() {
    assert!(matches!(probe(&[]), Err(PptxError::EmptyInput)));
    assert!(matches!(
        rasterize_page(&[], 0, 150),
        Err(PptxError::EmptyInput)
    ));
    assert!(matches!(extract_text(&[]), Err(PptxError::EmptyInput)));
    assert!(matches!(slide_count(&[]), Err(PptxError::EmptyInput)));
}

#[test]
fn invalid_dpi_rejected_before_parse() {
    assert!(matches!(
        rasterize_page(b"not a pptx", 0, 0),
        Err(PptxError::InvalidDpi(0))
    ));
    assert!(matches!(
        rasterize_all(b"not a pptx", 0),
        Err(PptxError::InvalidDpi(0))
    ));
}

#[test]
fn garbage_bytes_surface_parse_error() {
    assert!(matches!(probe(b"not a pptx"), Err(PptxError::Parse(_))));
    assert!(matches!(
        extract_text(b"not a pptx"),
        Err(PptxError::Parse(_))
    ));
    assert!(matches!(
        slide_count(b"not a pptx"),
        Err(PptxError::Parse(_))
    ));
}

#[test]
fn fixture_probes_and_extracts() {
    let meta = probe(PPTX).expect("probe fixture pptx");
    assert_eq!(meta.slides, 8);
    assert_eq!(slide_count(PPTX).expect("slide count"), meta.slides);
    let text = extract_text(PPTX).expect("extract text");
    assert!(text.contains("Sample Presentation"));
    assert!(text.contains("Agenda"));
}

#[test]
fn slide_count_matches_probe_slides() {
    let meta = probe(PPTX).expect("probe");
    let slides = slide_count(PPTX).expect("slide count");
    assert_eq!(slides, meta.slides);
    assert!(slides >= 1);
}

#[test]
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn rasterize_page_and_all_cover_fixture() {
    let pages = bevy_document_extend::pptx::page_count(PPTX).expect("page count");
    assert!(pages >= 1);
    for page in 0..pages {
        let buf = rasterize_page(PPTX, page, 72).expect("rasterize page");
        assert!(buf.width > 0 && buf.height > 0);
        assert_eq!(buf.rgba.len(), buf.width as usize * buf.height as usize * 4);
    }
    assert!(matches!(
        rasterize_page(PPTX, pages, 72),
        Err(PptxError::PageOutOfRange { .. })
    ));
    let all = rasterize_all(PPTX, 72).expect("rasterize all");
    assert_eq!(all.len(), pages);
}

#[test]
#[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
fn render_without_pdf_backend_reports_no_backend() {
    assert!(matches!(
        bevy_document_extend::pptx::page_count(PPTX),
        Err(PptxError::NoBackend)
    ));
    assert!(matches!(
        rasterize_page(PPTX, 0, 72),
        Err(PptxError::NoBackend)
    ));
}
