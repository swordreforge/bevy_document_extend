#![cfg(feature = "docx-rdocx")]

use bevy_document_extend::docx::DocxError;
use bevy_document_extend::docx::{
    available_backends, extract_text, probe, rasterize_all, rasterize_page,
};

#[test]
fn backends_advertised() {
    let backends = available_backends();
    assert!(!backends.is_empty());
    #[cfg(feature = "docx-rdocx")]
    assert!(backends.contains(&"rdocx"));
}

#[test]
fn empty_input_rejected() {
    assert!(matches!(probe(&[]), Err(DocxError::EmptyInput)));
    assert!(matches!(
        rasterize_page(&[], 0, 150),
        Err(DocxError::EmptyInput)
    ));
    assert!(matches!(extract_text(&[]), Err(DocxError::EmptyInput)));
}

#[test]
fn invalid_dpi_rejected_before_parse() {
    assert!(matches!(
        rasterize_page(b"not a docx", 0, 0),
        Err(DocxError::InvalidDpi(0))
    ));
}

#[test]
fn garbage_bytes_surface_parse_error() {
    assert!(matches!(
        rasterize_page(b"not a docx", 0, 150),
        Err(DocxError::Parse(_)
            | DocxError::Layout(_)
            | DocxError::Render(_)
            | DocxError::PageOutOfRange { .. })
    ));
}

#[test]
fn roundtrip_generated_doc_probes_and_extracts() {
    let mut doc = rdocx::Document::new();
    doc.add_paragraph("hello bevy");
    let mut bytes = doc.to_bytes().expect("serialize docx");
    let meta = probe(&bytes).expect("probe generated docx");
    assert!(meta.paragraphs >= 1);
    assert!(meta.words >= 2);
    let text = extract_text(&bytes).expect("extract text");
    assert!(text.contains("hello bevy"));
    bytes.clear();
}

#[test]
fn page_count_matches_probe_pages() {
    let mut doc = rdocx::Document::new();
    doc.add_paragraph("hello bevy count");
    let bytes = doc.to_bytes().expect("serialize docx");
    let meta = probe(&bytes).expect("probe");
    let pages = bevy_document_extend::docx::page_count(&bytes).expect("page count");
    assert_eq!(pages, meta.pages);
    assert!(pages >= 1);
}

#[test]
fn rasterize_page_and_all_cover_generated_doc() {
    let mut doc = rdocx::Document::new();
    doc.add_paragraph("hello bevy raster");
    let bytes = doc.to_bytes().expect("serialize docx");
    let pages = bevy_document_extend::docx::page_count(&bytes).expect("page count");
    assert!(pages >= 1);
    for page in 0..pages {
        let buf = rasterize_page(&bytes, page, 72).expect("rasterize page");
        assert!(buf.width > 0 && buf.height > 0);
        assert_eq!(buf.rgba.len(), buf.width as usize * buf.height as usize * 4);
    }
    assert!(matches!(
        rasterize_page(&bytes, pages, 72),
        Err(DocxError::PageOutOfRange { .. })
    ));
    let all = rasterize_all(&bytes, 72).expect("rasterize all");
    assert_eq!(all.len(), pages);
}

#[test]
fn rasterize_all_rejects_bad_input() {
    assert!(matches!(rasterize_all(&[], 72), Err(DocxError::EmptyInput)));
    assert!(matches!(
        rasterize_all(b"not a docx", 0),
        Err(DocxError::InvalidDpi(0))
    ));
}
