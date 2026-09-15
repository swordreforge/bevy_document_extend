//! XLSX viewer example: `cargo run --example xlsx_viewer --features viewer,xlsx-calamine`.
//!
//! Requires the `viewer` feature (windowed UI stack) plus the XLSX backend.

use bevy::prelude::*;
use bevy_document_extend::view_xlsx;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, view_xlsx("tests/exp/xlsx/sample100.xlsx")))
        .run();
}
