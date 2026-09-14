#![cfg(feature = "docx-rdocx")]

use bevy_document_extend::docx::DocxError;
use bevy_document_extend::docx::{available_backends, extract_text, probe, rasterize_page};

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
