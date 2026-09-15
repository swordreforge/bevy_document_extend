#![cfg(feature = "docx-office")]

use bevy_document_extend::docx::DocxError;
use bevy_document_extend::docx::{
    available_backends, extract_text, probe, rasterize_all, rasterize_page,
};
use std::io::{Cursor, Write};

/// Minimal DOCX bytes via docx-rs builder (dev-dependency): one paragraph
/// with `text`. Mirrors the helper in office2pdf's own `docx_tests.rs`.
fn build_docx(text: &str) -> Vec<u8> {
    let paragraph = docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text(text));
    let docx = docx_rs::Docx::new().add_paragraph(paragraph);
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

/// Minimal DOCX via raw zip for probe/extract-only assertions (no render):
/// hand-rolled `word/document.xml` with full control over paragraphs and
/// tables. office2pdf's docx-rs parser needs the full package shell, so
/// render assertions always go through [`build_docx`] instead.
fn build_docx_raw(body: &str) -> Vec<u8> {
    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="720" w:footer="720"/></w:sectPr></w:body>
</w:document>"#
    );
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
<Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
</Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    // docx-rs's reader and writer disagree on zip versions (8.x vs 0.6):
    // deflated entries written by 0.6 fail its parse. Stored (uncompressed)
    // entries read fine, and our probe/extract path only needs
    // `word/document.xml` anyway.
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, content) in [
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", rels),
            ("word/document.xml", &document_xml),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    buf.into_inner()
}

fn paragraph_xml(text: &str) -> String {
    format!(r#"<w:p><w:r><w:t>{text}</w:t></w:r></w:p>"#)
}

#[test]
fn backends_advertised() {
    let backends = available_backends();
    assert!(!backends.is_empty());
    #[cfg(feature = "docx-office")]
    assert!(backends.contains(&"office2pdf"));
}

#[test]
fn empty_input_rejected() {
    assert!(matches!(probe(&[]), Err(DocxError::EmptyInput)));
    assert!(matches!(
        rasterize_page(&[], 0, 150),
        Err(DocxError::EmptyInput)
    ));
    assert!(matches!(extract_text(&[]), Err(DocxError::EmptyInput)));
    assert!(matches!(
        bevy_document_extend::docx::to_pdf(&[]),
        Err(DocxError::EmptyInput)
    ));
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
            | DocxError::Convert(_)
            | DocxError::Render(_)
            | DocxError::PageOutOfRange { .. })
    ));
}

#[test]
fn probe_xml_stats_on_raw_fixture() {
    // XML-level counts bypass conversion: hand-rolled document.xml with
    // exact paragraph/table control.
    use bevy_document_extend::docx::OfficeDocxBackend;
    let body = format!(
        "{}{}",
        paragraph_xml("hello bevy"),
        paragraph_xml("second paragraph here")
    );
    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body}</w:body>
</w:document>"#
    );
    let (paragraphs, tables, texts) = OfficeDocxBackend::analyze(&document_xml);
    assert_eq!(paragraphs, 2);
    assert_eq!(tables, 0);
    let words = texts.join(" ").split_whitespace().count();
    assert!(words >= 5, "words {words}");
    let bytes = build_docx_raw(&body);
    let text = extract_text(&bytes).expect("extract text");
    assert!(text.contains("hello bevy"));
}

#[test]
fn probe_counts_and_extracts_generated_doc() {
    // Full package end-to-end (stats + converted page count).
    let full = build_docx("hello bevy");
    let meta = probe(&full).expect("probe generated docx");
    assert!(meta.paragraphs >= 1, "paragraphs {}", meta.paragraphs);
    assert!(meta.words >= 2, "words {}", meta.words);
    assert!(meta.pages >= 1, "pages {}", meta.pages);
    let text = extract_text(&full).expect("extract text");
    assert!(text.contains("hello bevy"));
}

#[test]
fn probe_counts_tables() {
    let cell = || {
        docx_rs::TableCell::new().add_paragraph(
            docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("cell text")),
        )
    };
    let table = docx_rs::Table::new(vec![docx_rs::TableRow::new(vec![cell(), cell()])]);
    let paragraph = docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("before table"));
    let docx = docx_rs::Docx::new()
        .add_paragraph(paragraph)
        .add_table(table);
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    let bytes = cursor.into_inner();
    let meta = probe(&bytes).expect("probe doc with table");
    assert_eq!(meta.tables, 1);
    assert!(meta.paragraphs >= 2, "paragraphs {}", meta.paragraphs);
    let text = extract_text(&bytes).expect("extract text");
    assert!(text.contains("cell text"));
}

#[test]
fn roundtrip_builder_doc_probes_and_extracts() {
    let bytes = build_docx("hello bevy builder");
    let meta = probe(&bytes).expect("probe builder docx");
    assert!(meta.paragraphs >= 1);
    assert!(meta.words >= 3);
    let text = extract_text(&bytes).expect("extract text");
    assert!(text.contains("hello bevy builder"));
}

#[test]
fn page_count_matches_probe_pages() {
    let bytes = build_docx("hello bevy count");
    let meta = probe(&bytes).expect("probe");
    let pages = bevy_document_extend::docx::page_count(&bytes).expect("page count");
    assert_eq!(pages, meta.pages);
    assert!(pages >= 1);
}

#[test]
fn to_pdf_converts_generated_doc() {
    let bytes = build_docx("hello bevy pdf");
    let pdf = bevy_document_extend::docx::to_pdf(&bytes).expect("convert to pdf");
    assert!(pdf.starts_with(b"%PDF"), "not a pdf: {:?}", &pdf[..8]);
}

#[test]
fn rasterize_page_and_all_cover_generated_doc() {
    let bytes = build_docx("hello bevy raster");
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

#[test]
fn second_conversion_hits_cache() {
    let bytes = build_docx("cache probe content");
    let first = bevy_document_extend::docx::to_pdf(&bytes).expect("first convert");
    let second = bevy_document_extend::docx::to_pdf(&bytes).expect("second convert");
    assert_eq!(first, second);
}
