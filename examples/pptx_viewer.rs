//! PPTX viewer example: `cargo run --example pptx_viewer --features viewer,pptx-office`.
//!
//! Requires the `viewer` feature (windowed UI stack) plus the PPTX backend.

use bevy::prelude::*;
use bevy_document_extend::view_pptx;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, view_pptx("tests/exp/pptx/sample.pptx")))
        .run();
}
