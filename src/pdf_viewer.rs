//! One-line PDF viewer: parse args, probe, rasterize, run the Bevy app.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build and run the PDF viewer from CLI args (`path [page] [dpi] [--fit | --no-fit]`).
///
/// This collapses the whole probe → rasterize → `App` wiring into one call,
/// so examples (and Bevy users embedding the viewer) need nothing between
/// `App::new()` and `.run()` except their own plugins:
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_document_extend::view_pdf;
///
/// let (plugin, source) = view_pdf("tests/exp/pdf/sample.pdf");
/// App::new()
///     .add_plugins((DefaultPlugins, plugin))
///     .insert_resource(source)
///     .run();
/// ```
pub fn view_pdf(default_path: &str) -> (DocumentViewerPlugin, DocumentSource) {
    let args = ViewerArgs::parse(default_path);
    let bytes = args.read_bytes();

    let meta = crate::probe(&bytes).expect("probe pdf");
    let info = format!("{} pages, version {}", meta.pages, meta.version);
    println!("{}: {info}", args.path);

    // Contract: `probe` guarantees `pages >= 1`, so no underflow guard here.
    let page = args.page.min(meta.pages - 1);
    let bytes_for_render = bytes.clone();
    let first = crate::rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize pdf page");
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
            crate::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
        }),
    )
}
