//! Minimal repro over the real files in `tests/exp/`.
//!
//! Each test embeds its fixture with `include_bytes!`, so it runs offline
//! with zero setup. Run with default features:
//! `cargo test --test exp_real`.

static PDF: &[u8] = include_bytes!(
    "exp/pdf/researcher-paper-意向残余、淤积动力学与节点涌现：本原信息的一种形式理论.pdf"
);
static DOCX: &[u8] = include_bytes!("exp/docx/sample3.docx");
static XLSX: &[u8] = include_bytes!("exp/xlsx/sample100.xlsx");
#[cfg(feature = "pptx-office")]
static PPTX: &[u8] = include_bytes!("exp/pptx/sample.pptx");

#[test]
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn pdf_real_probes_and_renders() {
    let meta = bevy_document_extend::probe(PDF).expect("probe real pdf");
    assert_eq!(meta.pages, 13);
    let buf = bevy_document_extend::rasterize_page(PDF, 0, 72).expect("rasterize p0");
    // A4 @72dpi ≈ 595x842; hayro/zpdf round sub-pixel edges differently (±1px).
    assert!((594..=597).contains(&buf.width), "width {}", buf.width);
    assert!((840..=843).contains(&buf.height), "height {}", buf.height);
    let hi = bevy_document_extend::rasterize_page(PDF, 0, 150).expect("rasterize p0 @150");
    assert!(hi.width > buf.width && hi.height > buf.height);
}

#[test]
#[cfg(feature = "docx-office")]
fn docx_real_probes_extracts_and_renders() {
    let meta = bevy_document_extend::docx::probe(DOCX).expect("probe real docx");
    assert_eq!(meta.pages, 180);
    assert!(meta.paragraphs >= 2000);
    assert!(meta.words >= 60000);
    let text = bevy_document_extend::docx::extract_text(DOCX).expect("extract text");
    assert!(text.contains("Lorem ipsum"));
    let buf = bevy_document_extend::docx::rasterize_page(DOCX, 0, 72).expect("rasterize p0");
    assert!(buf.width > 0 && buf.height > 0);
    assert_eq!(buf.rgba.len(), buf.width as usize * buf.height as usize * 4);
}

#[test]
#[cfg(feature = "xlsx-calamine")]
fn xlsx_real_probes_and_extracts() {
    let meta = bevy_document_extend::xlsx::probe(XLSX).expect("probe real xlsx");
    assert_eq!(meta.sheets, vec!["Sheet1".to_string()]);
    assert_eq!((meta.rows, meta.cols, meta.non_empty_cells), (100, 3, 300));
    let text = bevy_document_extend::xlsx::extract_text(XLSX, None).expect("extract text");
    assert!(text.contains("Name\tDOB\tDate Added"));
    assert!(text.contains("Joe Bloggs"));
}

#[test]
#[cfg(all(
    feature = "xlsx-calamine",
    feature = "xlsx-office2pdf",
    any(feature = "pdf-hayro", feature = "pdf-zpdf")
))]
fn xlsx_real_renders_through_office2pdf() {
    let pages = bevy_document_extend::xlsx::page_count(XLSX).expect("page count");
    assert_eq!(pages, 3);
    let buf = bevy_document_extend::xlsx::rasterize_page(XLSX, 0, 72).expect("rasterize p0");
    assert_eq!((buf.width, buf.height), (612, 792));
    let all = bevy_document_extend::xlsx::rasterize_all(XLSX, 72).expect("rasterize all");
    assert_eq!(all.len(), pages);
}

#[test]
#[cfg(feature = "pptx-office")]
fn pptx_real_probes_and_extracts() {
    let meta = bevy_document_extend::pptx::probe(PPTX).expect("probe real pptx");
    assert_eq!(meta.slides, 8);
    let text = bevy_document_extend::pptx::extract_text(PPTX).expect("extract text");
    assert!(text.contains("Sample Presentation"));
    assert!(text.contains("Agenda"));
}

#[test]
#[cfg(all(
    feature = "pptx-office",
    feature = "pptx-office2pdf",
    any(feature = "pdf-hayro", feature = "pdf-zpdf")
))]
fn pptx_real_renders_through_office2pdf() {
    let pages = bevy_document_extend::pptx::page_count(PPTX).expect("page count");
    assert!(pages >= 1);
    let buf = bevy_document_extend::pptx::rasterize_page(PPTX, 0, 72).expect("rasterize p0");
    assert!(buf.width > 0 && buf.height > 0);
    assert_eq!(buf.rgba.len(), buf.width as usize * buf.height as usize * 4);
    let all = bevy_document_extend::pptx::rasterize_all(PPTX, 72).expect("rasterize all");
    assert_eq!(all.len(), pages);
}
