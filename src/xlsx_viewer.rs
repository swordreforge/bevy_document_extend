//! One-line XLSX viewer: path in, viewer out.

use crate::viewer::{DocumentSource, DocumentViewerPlugin, ViewerArgs};
use bevy::prelude::*;

/// Build a ready-to-add XLSX viewer plugin from CLI args.
///
/// Demo/prototype sugar; see [`crate::view_pdf`] for the contract.
/// Production code should use [`DocumentViewerPlugin`] + `AssetServer` directly.
///
/// The optional `show_hud` toggles the bottom info bar (on by default);
/// `--no-hud` on the command line overrides it.
pub fn view_xlsx(default_path: impl Into<String>) -> impl Plugin {
    view_xlsx_with(default_path, true)
}

/// [`view_xlsx`] with an explicit info-bar choice.
pub fn view_xlsx_with(default_path: impl Into<String>, show_hud: bool) -> impl Plugin {
    struct XlsxViewer {
        default_path: String,
        show_hud: bool,
    }

    impl Plugin for XlsxViewer {
        fn build(&self, app: &mut App) {
            let args = ViewerArgs::parse(&self.default_path);
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
            let first = crate::xlsx::rasterize_page_to_bevy(&bytes, page, args.dpi)
                .expect("rasterize xlsx page");
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
                crate::xlsx::rasterize_page_to_bevy(&bytes_for_render, page, dpi).ok()
            }));
        }
    }

    XlsxViewer {
        default_path: default_path.into(),
        show_hud,
    }
}
