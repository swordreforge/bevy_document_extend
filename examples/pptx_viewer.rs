#[cfg(not(feature = "pptx-office"))]
fn main() {
    eprintln!("pptx_viewer requires the pptx backend feature: pptx-office");
}

#[cfg(feature = "pptx-office")]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::view_pptx;
    App::new()
        .add_plugins((DefaultPlugins, view_pptx("tests/exp/pptx/sample.pptx")))
        .run();
}
