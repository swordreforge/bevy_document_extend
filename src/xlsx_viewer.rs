//! One-line XLSX viewer: parse args, probe, rasterize, run the Bevy app.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build the XLSX viewer from CLI args (`path [page] [dpi] [--fit | --no-fit]`).
///
/// Returns the `(plugin, source)` pair; see [`crate::view_pdf`] for usage.
pub fn view_xlsx(default_path: &str) -> (DocumentViewerPlugin, DocumentSource) {
    let args = ViewerArgs::parse(default_path);
    let bytes = args.read_bytes();

    let meta = crate::xlsx::probe(&bytes).expect("probe xlsx");
    let pages = crate::xlsx::page_count(&bytes).expect("xlsx page count");
    let info = format!(
        "sheets {:?}, {}x{} range, {} non-empty cells",
        meta.sheets, meta.rows, meta.cols, meta.non_empty_cells
    );
    println!("{}: {info}", args.path);

    // Contract: `page_count` guarantees `>= 1`, so no underflow guard here.
    let page = args.page.min(pages - 1);
    let bytes_for_render = bytes.clone();
    let first =
        crate::xlsx::rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize xlsx page");
    (
        DocumentViewerPlugin {
            title: args.path,
            probe: info,
            pages,
            page,
            dpi: args.dpi,
            fit: args.fit,
        },
        DocumentSource::new(first, move |page, dpi| {
            crate::xlsx::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
        }),
    )
}
