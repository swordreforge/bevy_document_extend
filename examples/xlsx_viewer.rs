//! XLSX viewer: open a `.xlsx` workbook in a scrollable Bevy window.
//!
//! Run (from the crate root):
//!
//! ```sh
//! cargo run --release --example xlsx_viewer --features viewer,xlsx-calamine
//! ```
//!
//! Open your own file by passing a path (plus an optional start page and
//! render resolution):
//!
//! ```sh
//! cargo run --release --example xlsx_viewer --features viewer,xlsx-calamine -- budget.xlsx
//! cargo run --release --example xlsx_viewer --features viewer,xlsx-calamine -- budget.xlsx 1
//! cargo run --release --example xlsx_viewer --features viewer,xlsx-calamine -- budget.xlsx 0 200
//! ```
//!
//! Flags: `--fit` / `--no-fit` force fit-to-width on/off (without flags,
//! omitting `dpi` means fit, passing `dpi` means fixed zoom), `--no-hud`
//! hides the bottom info bar.
//!
//! Controls: wheel scrolls through all pages, `Left`/`Right` jump one page,
//! `Home`/`End` jump to first/last page, `Ctrl`+wheel (or pinch) zooms,
//! `Up`/`Down` zoom stepwise, `0` re-fits to width, `Q` quits.
//!
//! What this wires up: [`DefaultPlugins`] provides the window, renderer and
//! UI stack; [`view_xlsx`](bevy_document_extend::view_xlsx) reads the file,
//! renders sheets via calamine + office2pdf, and installs the viewer plugin.
//! Swap in your own path or call `view_xlsx_with(path, show_hud)` to hide
//! the info bar programmatically.

use bevy::prelude::*;
use bevy_document_extend::view_xlsx;

fn main() {
    App::new()
        // The bundled sample; replace with any path, or pass one on the
        // command line (see the module docs above).
        .add_plugins((DefaultPlugins, view_xlsx("tests/exp/xlsx/sample100.xlsx")))
        .run();
}
