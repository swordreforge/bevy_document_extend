#[cfg(not(feature = "xlsx-calamine"))]
fn main() {
    eprintln!("xlsx_viewer requires the xlsx backend feature: xlsx-calamine");
}

#[cfg(feature = "xlsx-calamine")]
fn main() {
    use bevy::prelude::*;
    use bevy_document_extend::view_xlsx;
    let (plugin, source) = view_xlsx("tests/exp/xlsx/sample100.xlsx");
    App::new()
        .add_plugins((DefaultPlugins, plugin))
        .insert_resource(source)
        .run();
}
