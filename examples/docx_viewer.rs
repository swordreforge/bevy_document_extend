use bevy::prelude::*;
use bevy_document_extend::docx::{probe, rasterize_page_to_bevy};

#[derive(Resource)]
struct DocRequest {
    bytes: Vec<u8>,
    page: usize,
    dpi: u32,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: docx_viewer <file.docx> [page] [dpi]");
    let page = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let dpi = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(150);
    let bytes = std::fs::read(&path).expect("read docx file");

    let meta = probe(&bytes).expect("probe docx");
    println!(
        "{}: {} pages, {} paragraphs, {} tables, {} words",
        path, meta.pages, meta.paragraphs, meta.tables, meta.words
    );

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(DocRequest { bytes, page, dpi })
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, req: Res<DocRequest>) {
    let image = rasterize_page_to_bevy(&req.bytes, req.page, req.dpi).expect("rasterize docx page");
    let handle = images.add(image);
    commands.spawn(Camera2d);
    commands.spawn(Sprite::from_image(handle));
}
