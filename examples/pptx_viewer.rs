//! PPTX viewer: open a `.pptx` deck in a scrollable Bevy window.
//!
//! Run (from the crate root):
//!
//! ```sh
//! cargo run --release --example pptx_viewer --features viewer,pptx-office
//! ```
//!
//! Open your own file by passing a path (plus an optional start slide and
//! render resolution):
//!
//! ```sh
//! cargo run --release --example pptx_viewer --features viewer,pptx-office -- deck.pptx
//! cargo run --release --example pptx_viewer --features viewer,pptx-office -- deck.pptx 3
//! cargo run --release --example pptx_viewer --features viewer,pptx-office -- deck.pptx 0 200
//! ```
//!
//! Flags: `--fit` / `--no-fit` force fit-to-width on/off (without flags,
//! omitting `dpi` means fit, passing `dpi` means fixed zoom), `--no-hud`
//! hides the bottom info bar.
//!
//! Controls: wheel scrolls through all slides, `Left`/`Right` jump one
//! slide, `Home`/`End` jump to first/last slide, `Ctrl`+wheel (or pinch)
//! zooms, `Up`/`Down` zoom stepwise, `0` re-fits to width, `Q` quits.
//!
//! What this wires up: [`DefaultPlugins`] provides the window, renderer and
//! UI stack; [`view_pptx`](bevy_document_extend::view_pptx) reads the file,
//! renders slides via office2pdf, and installs the viewer plugin. Swap in
//! your own path or call `view_pptx_with(path, show_hud)` to hide the info
//! bar programmatically.

use bevy::prelude::*;
use bevy_document_extend::view_pptx;

fn main() {
    App::new()
        // The bundled sample; replace with any path, or pass one on the
        // command line (see the module docs above).
        .add_plugins((DefaultPlugins, view_pptx("tests/exp/pptx/sample.pptx")))
        .run();
}
