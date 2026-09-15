#[cfg(not(feature = "xlsx-calamine"))]
fn main() {
    eprintln!("xlsx_viewer requires the xlsx backend feature: xlsx-calamine");
}

#[cfg(feature = "xlsx-calamine")]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
    use bevy_document_extend::xlsx::{page_count, probe, rasterize_page_to_bevy};

    let args = ViewerArgs::parse("tests/exp/xlsx/sample100.xlsx");
    let bytes = std::fs::read(&args.path).unwrap_or_else(|e| panic!("read {}: {e}", args.path));

    let meta = probe(&bytes).expect("probe xlsx");
    let pages = page_count(&bytes).expect("xlsx page count");
    let info = format!(
        "sheets {:?}, {}x{} range, {} non-empty cells",
        meta.sheets, meta.rows, meta.cols, meta.non_empty_cells
    );
    println!("{}: {info}", args.path);
    // Contract: `page_count` guarantees `>= 1`, so no underflow guard here.
    let page = args.page.min(pages - 1);
    let bytes_for_render = bytes.clone();
    let first = rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize xlsx page");
    App::new()
        .add_plugins((
            DefaultPlugins,
            DocumentViewerPlugin {
                title: args.path,
                probe: info,
                pages,
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
