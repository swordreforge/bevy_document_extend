#[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
fn main() {
    eprintln!("pdf_viewer requires a pdf backend feature: pdf-hayro or pdf-zpdf");
}

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::view_pdf;
    App::new()
        .add_plugins((DefaultPlugins, view_pdf("tests/exp/pdf/sample.pdf")))
        .run();
}
