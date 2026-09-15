//! DOCX viewer example: `cargo run --example docx_viewer --features viewer,docx-rdocx`.
//!
//! Requires the `viewer` feature (windowed UI stack) plus the DOCX backend.

use bevy::prelude::*;
use bevy_document_extend::view_docx;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, view_docx("tests/exp/docx/sample3.docx")))
        .run();
}
