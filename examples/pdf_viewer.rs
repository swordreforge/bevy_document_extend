#[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
fn main() {
    eprintln!("pdf_viewer requires a pdf backend feature: pdf-hayro or pdf-zpdf");
}

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
mod app {
    use bevy::prelude::*;
    use bevy_document_extend::{
        probe, rasterize_page_to_bevy,
        viewer::{DocumentViewer, DocumentViewerPlugin, RenderCallbackCell},
    };

    pub fn run() {
        let path = std::env::args().nth(1).unwrap_or_else(|| {
            "tests/exp/pdf/researcher-paper-意向残余、淤积动力学与节点涌现：本原信息的一种形式理论.pdf"
                .to_string()
        });
        let page = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let dpi = std::env::args()
            .nth(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(150);
        let fit = std::env::args().nth(3).is_none();
        let bytes = std::fs::read(&path).expect("read pdf file");

        let meta = probe(&bytes).expect("probe pdf");
        let info = format!("{} pages, version {}", meta.pages, meta.version);
        println!("{path}: {info}");
        let page = page.min(meta.pages.max(1) - 1);
        let bytes_for_render = bytes.clone();
        let first = rasterize_page_to_bevy(&bytes, page, dpi).expect("rasterize pdf page");
        App::new()
            .add_plugins((
                DefaultPlugins,
                DocumentViewerPlugin(DocumentViewer {
                    title: path,
                    probe: info,
                    pages: meta.pages,
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

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn main() {
    app::run();
}
