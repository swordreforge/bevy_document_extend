#[cfg(not(feature = "docx-rdocx"))]
fn main() {
    eprintln!("docx_viewer requires the docx backend feature: docx-rdocx");
}

#[cfg(feature = "docx-rdocx")]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::docx::{probe, rasterize_page_to_bevy};
    use bevy_document_extend::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};

    let args = ViewerArgs::parse("tests/exp/docx/sample3.docx");
    let bytes = std::fs::read(&args.path).unwrap_or_else(|e| panic!("read {}: {e}", args.path));

    let meta = probe(&bytes).expect("probe docx");
    let info = format!(
        "{} pages, {} paragraphs, {} tables, {} words",
        meta.pages, meta.paragraphs, meta.tables, meta.words
    );
    println!("{}: {info}", args.path);
    // Contract: `probe` guarantees `pages >= 1`, so no underflow guard here.
    let page = args.page.min(meta.pages - 1);
    let bytes_for_render = bytes.clone();
    let first = rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize docx page");
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
