//! One-line PDF viewer: path in, viewer out.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build a ready-to-add viewer plugin from CLI args (`path [page] [dpi] [--fit | --no-fit]`).
///
/// Demo/prototype sugar: reads the file, probes, and rasterizes the first
/// page synchronously inside [`Plugin::build`], then installs
/// [`DocumentViewerPlugin`] + [`DocumentSource`] for you. The caller only
/// passes a path — no tuple, no manual `insert_resource`:
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_document_extend::view_pdf;
///
/// App::new()
///     .add_plugins((DefaultPlugins, view_pdf("tests/exp/pdf/sample.pdf")))
///     .run();
/// ```
///
/// Production code should use [`DocumentViewerPlugin`] + `AssetServer`
/// directly instead of this fs-reading shortcut.
pub fn view_pdf(default_path: impl Into<String>) -> impl Plugin {
    struct PdfViewer {
        default_path: String,
    }

    impl Plugin for PdfViewer {
        fn build(&self, app: &mut App) {
            let args = ViewerArgs::parse(&self.default_path);
            let bytes = args.read_bytes();

            let meta = crate::probe(&bytes).expect("probe pdf");
            let info = format!("{} pages, version {}", meta.pages, meta.version);
            println!("{}: {info}", args.path);

            // Contract: `probe` guarantees `pages >= 1`, so no underflow guard here.
            let page = args.page.min(meta.pages - 1);
            let bytes_for_render = bytes.clone();
            let first =
                crate::rasterize_page_to_bevy(&bytes, page, args.dpi).expect("rasterize pdf page");
            app.add_plugins(DocumentViewerPlugin {
                title: args.path,
                probe: info,
                pages: meta.pages,
                page,
                dpi: args.dpi,
                fit: args.fit,
            });
            app.insert_resource(DocumentSource::new(first, move |page, dpi| {
                crate::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
            }));
        }
    }

    PdfViewer {
        default_path: default_path.into(),
    }
}
