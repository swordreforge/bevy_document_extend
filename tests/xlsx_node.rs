#![cfg(feature = "xlsx-calamine")]

use bevy_document_extend::xlsx::XlsxError;
use bevy_document_extend::xlsx::{
    available_backends, extract_text, probe, rasterize_all, rasterize_page, sheet_names,
};

fn make_workbook() -> Vec<u8> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.write_string(0, 0, "hello").expect("write a1");
    sheet.write_string(0, 1, "bevy").expect("write b1");
    sheet.write_number(1, 0, 42.0).expect("write a2");
    workbook.save_to_buffer().expect("serialize xlsx")
}

#[test]
fn backends_advertised() {
    let backends = available_backends();
    assert!(!backends.is_empty());
    #[cfg(feature = "xlsx-calamine")]
    assert!(backends.contains(&"calamine-office2pdf"));
}

#[test]
fn empty_input_rejected() {
    assert!(matches!(probe(&[]), Err(XlsxError::EmptyInput)));
    assert!(matches!(
        rasterize_page(&[], 0, 150),
        Err(XlsxError::EmptyInput)
    ));
    assert!(matches!(
        extract_text(&[], None),
        Err(XlsxError::EmptyInput)
    ));
    assert!(matches!(sheet_names(&[]), Err(XlsxError::EmptyInput)));
}

#[test]
fn invalid_dpi_rejected_before_parse() {
    assert!(matches!(
        rasterize_page(b"not an xlsx", 0, 0),
        Err(XlsxError::InvalidDpi(0))
    ));
    assert!(matches!(
        rasterize_all(b"not an xlsx", 0),
        Err(XlsxError::InvalidDpi(0))
    ));
}

#[test]
fn garbage_bytes_surface_parse_error() {
    assert!(matches!(probe(b"not an xlsx"), Err(XlsxError::Parse(_))));
    assert!(matches!(
        extract_text(b"not an xlsx", None),
        Err(XlsxError::Parse(_))
    ));
    assert!(matches!(
        sheet_names(b"not an xlsx"),
        Err(XlsxError::Parse(_))
    ));
}

#[test]
fn generated_workbook_probes_and_extracts() {
    let bytes = make_workbook();
    let meta = probe(&bytes).expect("probe generated xlsx");
    assert!(!meta.sheets.is_empty());
    assert!(meta.non_empty_cells >= 3);
    let names = sheet_names(&bytes).expect("sheet names");
    assert_eq!(names, meta.sheets);
    let text = extract_text(&bytes, None).expect("extract text");
    assert!(text.contains("hello"));
    assert!(text.contains("bevy"));
    assert!(text.contains("42"));
}

#[test]
fn unknown_sheet_rejected() {
    let bytes = make_workbook();
    assert!(matches!(
        extract_text(&bytes, Some("NoSuchSheet")),
        Err(XlsxError::SheetNotFound(_))
    ));
}

#[test]
#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn rasterize_page_and_all_cover_generated_workbook() {
    let bytes = make_workbook();
    let pages = bevy_document_extend::xlsx::page_count(&bytes).expect("page count");
    assert!(pages >= 1);
    for page in 0..pages {
        let buf = rasterize_page(&bytes, page, 72).expect("rasterize page");
        assert!(buf.width > 0 && buf.height > 0);
        assert_eq!(buf.rgba.len(), buf.width as usize * buf.height as usize * 4);
    }
    assert!(matches!(
        rasterize_page(&bytes, pages, 72),
        Err(XlsxError::PageOutOfRange { .. })
    ));
    let all = rasterize_all(&bytes, 72).expect("rasterize all");
    assert_eq!(all.len(), pages);
}

#[test]
#[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
fn render_without_pdf_backend_reports_no_backend() {
    let bytes = make_workbook();
    assert!(matches!(
        bevy_document_extend::xlsx::page_count(&bytes),
        Err(XlsxError::NoBackend)
    ));
    assert!(matches!(
        rasterize_page(&bytes, 0, 72),
        Err(XlsxError::NoBackend)
    ));
}
