//! One-line PPTX viewer: path in, viewer out.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build a ready-to-add PPTX viewer plugin from CLI args.
///
/// Demo/prototype sugar; see [`crate::view_pdf`] for the contract.
/// Production code should use [`DocumentViewerPlugin`] + `AssetServer` directly.
///
/// The optional `show_hud` toggles the bottom info bar (on by default);
/// `--no-hud` on the command line overrides it.
pub fn view_pptx(default_path: impl Into<String>) -> impl Plugin {
    view_pptx_with(default_path, true)
}

/// [`view_pptx`] with an explicit info-bar choice.
pub fn view_pptx_with(default_path: impl Into<String>, show_hud: bool) -> impl Plugin {
    struct PptxViewer {
        default_path: String,
        show_hud: bool,
    }

    impl Plugin for PptxViewer {
        fn build(&self, app: &mut App) {
            let args = ViewerArgs::parse(&self.default_path);
            let bytes = args.read_bytes();

            let meta = crate::pptx::probe(&bytes).expect("probe pptx");
            let pages = crate::pptx::page_count(&bytes).expect("pptx page count");
            let title = meta.title.as_deref().unwrap_or("");
            let info = if title.is_empty() {
                format!("{} slides", meta.slides)
            } else {
                format!("{} slides, {}", meta.slides, title)
            };
            println!("{}: {info}", args.path);

            // Contract: `page_count` guarantees `>= 1`, so no underflow guard here.
            let page = args.page.min(pages - 1);
            let bytes_for_render = bytes.clone();
            let first = crate::pptx::rasterize_page_to_bevy(&bytes, page, args.dpi)
                .expect("rasterize pptx page");
            // `--no-hud` on the command line overrides the builder default.
            let show_hud = !ViewerArgs::has_flag("--no-hud") && self.show_hud;
            app.add_plugins(DocumentViewerPlugin {
                title: args.path,
                probe: info,
                pages,
                page,
                dpi: args.dpi,
                fit: args.fit,
                show_hud,
            });
            app.insert_resource(DocumentSource::new(first, move |page, dpi| {
                crate::pptx::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
            }));
        }
    }

    PptxViewer {
        default_path: default_path.into(),
        show_hud,
    }
}
