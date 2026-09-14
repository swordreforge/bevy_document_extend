#[cfg(not(feature = "xlsx-calamine"))]
fn main() {
    eprintln!("xlsx_viewer requires the xlsx backend feature: xlsx-calamine");
}

#[cfg(feature = "xlsx-calamine")]
mod app {
    use bevy::prelude::*;
    use bevy_document_extend::xlsx::{probe, rasterize_page_to_bevy};

    #[derive(Resource)]
    pub struct DocRequest {
        pub bytes: Vec<u8>,
        pub page: usize,
        pub dpi: u32,
    }

    pub fn run() {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "tests/exp/xlsx/sample100.xlsx".to_string());
        let page = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let dpi = std::env::args()
            .nth(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(150);
        let bytes = std::fs::read(&path).expect("read xlsx file");

        let meta = probe(&bytes).expect("probe xlsx");
        println!(
            "{}: sheets {:?}, {}x{} range, {} non-empty cells",
            path, meta.sheets, meta.rows, meta.cols, meta.non_empty_cells
        );

        App::new()
            .add_plugins(DefaultPlugins)
            .insert_resource(DocRequest { bytes, page, dpi })
            .add_systems(Startup, setup)
            .run();
    }

    fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, req: Res<DocRequest>) {
        let image =
            rasterize_page_to_bevy(&req.bytes, req.page, req.dpi).expect("rasterize xlsx page");
        let handle = images.add(image);
        commands.spawn(Camera2d);
        commands.spawn(Sprite::from_image(handle));
    }
}

#[cfg(feature = "xlsx-calamine")]
fn main() {
    app::run();
}
