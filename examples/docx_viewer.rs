#[cfg(not(feature = "docx-rdocx"))]
fn main() {
    eprintln!("docx_viewer requires the docx backend feature: docx-rdocx");
}
#[cfg(feature = "docx-rdocx")]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::view_docx;
    App::new()
        .add_plugins((DefaultPlugins, view_docx("tests/exp/docx/sample3.docx")))
        .run();
}
