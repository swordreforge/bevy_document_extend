#[cfg(not(feature = "docx-rdocx"))]
fn main() {
    eprintln!("docx_viewer requires the docx backend feature: docx-rdocx");
}

#[cfg(feature = "docx-rdocx")]
#[path = "viewer_common/mod.rs"]
mod viewer_common;

#[cfg(feature = "docx-rdocx")]
mod app {
    use super::viewer_common;
    use bevy_document_extend::docx::{probe, rasterize_page_to_bevy};

    pub fn run() {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "tests/exp/docx/sample3.docx".to_string());
        let page = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let dpi = std::env::args()
            .nth(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(150);
        let fit = std::env::args().nth(3).is_none();
        let bytes = std::fs::read(&path).expect("read docx file");

        let meta = probe(&bytes).expect("probe docx");
        let info = format!(
            "{} pages, {} paragraphs, {} tables, {} words",
            meta.pages, meta.paragraphs, meta.tables, meta.words
        );
        println!("{path}: {info}");
        let page = page.min(meta.pages.max(1) - 1);
        let bytes_for_render = bytes.clone();
        let first = rasterize_page_to_bevy(&bytes, page, dpi).expect("rasterize docx page");
        viewer_common::run(
            path,
            info,
            meta.pages,
            page,
            dpi,
            fit,
            first,
            move |page, dpi| rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok(),
        );
    }
}

#[cfg(feature = "docx-rdocx")]
fn main() {
    app::run();
}
