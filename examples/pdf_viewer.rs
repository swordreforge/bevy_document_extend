#[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
fn main() {
    eprintln!("pdf_viewer requires a pdf backend feature: pdf-hayro or pdf-zpdf");
}

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
mod app {
    use bevy::prelude::*;
    use bevy_document_extend::{probe, rasterize_page_to_bevy};

    #[derive(Resource)]
    pub struct DocRequest {
        pub bytes: Vec<u8>,
        pub page: usize,
        pub dpi: u32,
    }

    pub fn run() {
        let path = std::env::args().nth(1).unwrap_or_else(|| {
            "tests/exp/pdf/researcher-paper-意向残余、淤积动力学与节点涌现：本原信息的一种形式理论.pdf"
                .to_string()
        });
        let page = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let dpi = std::env::args()
            .nth(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(150);
        let bytes = std::fs::read(&path).expect("read pdf file");

        let meta = probe(&bytes).expect("probe pdf");
        println!("{}: {} pages, version {}", path, meta.pages, meta.version);

        App::new()
            .add_plugins(DefaultPlugins)
            .insert_resource(DocRequest { bytes, page, dpi })
            .add_systems(Startup, setup)
            .run();
    }

    fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, req: Res<DocRequest>) {
        let image =
            rasterize_page_to_bevy(&req.bytes, req.page, req.dpi).expect("rasterize pdf page");
        let handle = images.add(image);
        commands.spawn(Camera2d);
        commands.spawn(Sprite::from_image(handle));
    }
}

#[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
fn main() {
    app::run();
}
