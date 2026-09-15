//! PDF viewer example: `cargo run --example pdf_viewer --features viewer,pdf-hayro`.
//!
//! Requires the `viewer` feature (windowed UI stack) plus a PDF backend.

use bevy::prelude::*;
use bevy_document_extend::view_pdf;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, view_pdf("tests/exp/pdf/sample.pdf")))
        .run();
}
