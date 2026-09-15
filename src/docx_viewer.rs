//! One-line DOCX viewer: path in, viewer out.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build a ready-to-add DOCX viewer plugin from CLI args.
///
/// Demo/prototype sugar; see [`crate::view_pdf`] for the contract.
/// Production code should use [`DocumentViewerPlugin`] + `AssetServer` directly.
pub fn view_docx(default_path: impl Into<String>) -> impl Plugin {
    struct DocxViewer {
        default_path: String,
    }

    impl Plugin for DocxViewer {
        fn build(&self, app: &mut App) {
            let args = ViewerArgs::parse(&self.default_path);
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
            let first = crate::docx::rasterize_page_to_bevy(&bytes, page, args.dpi)
                .expect("rasterize docx page");
            app.add_plugins(DocumentViewerPlugin {
                title: args.path,
                probe: info,
                pages: meta.pages,
                page,
                dpi: args.dpi,
                fit: args.fit,
            });
            app.insert_resource(DocumentSource::new(first, move |page, dpi| {
                crate::docx::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
            }));
        }
    }

    DocxViewer {
        default_path: default_path.into(),
    }
}
