#[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
fn main() {
    eprintln!("pdf_viewer requires a pdf backend feature: pdf-hayro or pdf-zpdf");
}

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::{
        probe, rasterize_page_to_bevy,
        viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs},
    };

    let args = ViewerArgs::parse("tests/exp/pdf/sample.pdf");
    let bytes = std::fs::read(&args.path).unwrap_or_else(|e| panic!("read {}: {e}", args.path));

    let meta = probe(&bytes).expect("probe pdf");
    let info = format!("{} pages, version {}", meta.pages, meta.version);
    println!("{}: {info}", args.path);

    // Contract: `probe` guarantees `pages >= 1`, so no underflow guard here.
    let page = args.page.min(meta.pages - 1);
    let bytes_for_render = bytes.clone();
    let first = rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize pdf page");
    App::new()
        .add_plugins((
            DefaultPlugins,
            DocumentViewerPlugin {
                title: args.path,
                probe: info,
                pages: meta.pages,
                page,
                dpi: args.dpi,
                fit: args.fit,
            },
        ))
        .insert_resource(DocumentSource::new(first, move |page, dpi| {
            rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
        }))
        .run();
}
