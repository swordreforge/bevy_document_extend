//! One-line DOCX viewer: parse args, probe, rasterize, run the Bevy app.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build the DOCX viewer from CLI args (`path [page] [dpi] [--fit | --no-fit]`).
///
/// Returns the `(plugin, source)` pair; see [`crate::view_pdf`] for usage.
pub fn view_docx(default_path: &str) -> (DocumentViewerPlugin, DocumentSource) {
    let args = ViewerArgs::parse(default_path);
    let bytes = args.read_bytes();

    let meta = crate::docx::probe(&bytes).expect("probe docx");
    let info = format!(
        "{} pages, {} paragraphs, {} tables, {} words",
        meta.pages, meta.paragraphs, meta.tables, meta.words
    );
    println!("{}: {info}", args.path);

    // Contract: `probe` guarantees `pages >= 1`, so no underflow guard here.
    let page = args.page.min(meta.pages - 1);
    let bytes_for_render = bytes.clone();
    let first =
        crate::docx::rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize docx page");
    (
        DocumentViewerPlugin {
            title: args.path,
            probe: info,
            pages: meta.pages,
            page,
            dpi: args.dpi,
            fit: args.fit,
        },
        DocumentSource::new(first, move |page, dpi| {
            crate::docx::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
        }),
    )
}
