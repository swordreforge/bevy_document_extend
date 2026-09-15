#[cfg(not(feature = "xlsx-calamine"))]
fn main() {
    eprintln!("xlsx_viewer requires the xlsx backend feature: xlsx-calamine");
}

#[cfg(feature = "xlsx-calamine")]
mod app {
    use bevy::prelude::*;
    use bevy_document_extend::viewer::{DocumentViewer, DocumentViewerPlugin, RenderCallbackCell};
    use bevy_document_extend::xlsx::{page_count, probe, rasterize_page_to_bevy};

    pub fn run() {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "tests/exp/xlsx/sample100.xlsx".to_string());
        let page = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let dpi = std::env::args()
            .nth(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(150);
        let fit = std::env::args().nth(3).is_none();
        let bytes = std::fs::read(&path).expect("read xlsx file");

        let meta = probe(&bytes).expect("probe xlsx");
        let pages = page_count(&bytes).expect("xlsx page count");
        let info = format!(
            "sheets {:?}, {}x{} range, {} non-empty cells",
            meta.sheets, meta.rows, meta.cols, meta.non_empty_cells
        );
        println!("{path}: {info}");
        let page = page.min(pages.max(1) - 1);
        let bytes_for_render = bytes.clone();
        let first = rasterize_page_to_bevy(&bytes, page, dpi).expect("rasterize xlsx page");
        App::new()
            .add_plugins((
                DefaultPlugins,
                DocumentViewerPlugin(DocumentViewer {
                    title: path,
                    probe: info,
                    pages,
                    page,
                    dpi,
                    fit,
                    first,
                    render: RenderCallbackCell::new(move |page, dpi| {
                        rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
                    }),
                }),
            ))
            .run();
    }
}

#[cfg(feature = "xlsx-calamine")]
fn main() {
    app::run();
}
