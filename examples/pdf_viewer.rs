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

    let args = ViewerArgs::parse(
        "tests/exp/pdf/researcher-paper-意向残余、淤积动力学与节点涌现：本原信息的一种形式理论.pdf",
    );
    let bytes = std::fs::read(&args.path).expect("read pdf file");

    let meta = probe(&bytes).expect("probe pdf");
    let info = format!("{} pages, version {}", meta.pages, meta.version);
    println!("{}: {info}", args.path);
    let page = args.page.min(meta.pages.max(1) - 1);
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
