# bevy_document_extend

Pure-Rust document rendering for [Bevy](https://bevyengine.org), backed by
[`hayro`](https://crates.io/crates/hayro) / [`zpdf`](https://crates.io/crates/zpdf),
[`calamine`](https://crates.io/crates/calamine) + [`office2pdf`](https://crates.io/crates/office2pdf).
No C, no FFI, no LibreOffice.

Bevy 0.19 has no built-in document support — `bevy_asset` stops at images/audio,
and high-fidelity Office rendering elsewhere usually means linking LibreOfficeKit.
This crate probes document bytes and rasterizes pages straight into Bevy `Image`
assets, so the whole pipeline stays pure Rust on **native and wasm**.

## Features

- **Zero C/FFI** — no LibreOffice, no `bindgen`, no system toolchain at build time.
- **Feature-gated backends** — PDF / DOCX / XLSX are independent Cargo features;
  you only compile what you use.
- **Probe + rasterize** — `probe` reports page counts and metadata, `rasterize_page`
  returns raw RGBA, `rasterize_page_to_bevy` returns a ready-to-spawn Bevy `Image`.
- **Bevy-native textures** — linear filtering + mipmaps via `ImageSamplerDescriptor`,
  transparent page areas flattened onto paper white.
- **Viewer examples** — `pdf_viewer` / `docx_viewer` / `xlsx_viewer` share one shell:
  fit-to-width, wheel scroll, `Ctrl+wheel` continuous zoom, debounced re-raster.

## Installation

```toml
[dependencies]
# Default: hayro (PDF) + office2pdf (DOCX) + calamine/office2pdf (XLSX) + office2pdf (PPTX)
bevy_document_extend = "0.1.0-alpha.0"

# Or pick backends explicitly (default-features = false):
# bevy_document_extend = { version = "0.1.0-alpha.0", default-features = false, features = ["pdf-hayro"] }
```

| Feature                         | Format | Backend                    |
|---------------------------------|--------|----------------------------|
| `pdf-hayro` (default)           | PDF    | `hayro` rasterizer         |
| `pdf-zpdf`                      | PDF    | `zpdf` rasterizer          |
| `docx-office` (default)         | DOCX   | `office2pdf` → PDF → `hayro` |
| `xlsx-calamine-office2pdf` (default) | XLSX | `calamine` read + `office2pdf` → PDF → `hayro` |
| `pptx-office` (default)         | PPTX   | `office2pdf` → PDF → `hayro` |

All three Office formats share one render chain (`office -> office2pdf ->
pdf -> hayro`), so every `*-office*` feature renders out of the box with no
extra setup. DOCX `probe`/`extract_text` additionally read the OPC package
directly (`zip` + `quick-xml`), so metadata never pays a conversion.

## Usage

```rust
use bevy::prelude::*;
use bevy_document_extend::{probe, rasterize_page_to_bevy};

fn load_pdf(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let bytes = std::fs::read("docs/report.pdf").unwrap();
    let meta = probe(&bytes).unwrap();
    println!("{} pages, version {}", meta.pages, meta.version);

    let page = rasterize_page_to_bevy(&bytes, 0, 150).unwrap();
    let handle = images.add(page);
    commands.spawn(ImageNode::new(handle));
}
```

DOCX and XLSX follow the same shape under their own modules:

```rust
use bevy_document_extend::docx;
use bevy_document_extend::xlsx;

let meta = docx::probe(&bytes).unwrap();
let first = docx::rasterize_page_to_bevy(&bytes, 0, 150).unwrap();

let meta = xlsx::probe(&bytes).unwrap();
let first = xlsx::rasterize_page_to_bevy(&bytes, 0, 150).unwrap();
```

### Viewer examples

```sh
cargo run --example pdf_viewer -- docs/report.pdf [page] [dpi]
cargo run --example docx_viewer -- docs/report.docx [page] [dpi]
cargo run --example xlsx_viewer -- docs/sheet.xlsx [page] [dpi]
```

Controls: `←/→` page · wheel scroll · `Ctrl+wheel`/pinch zoom · `↑/↓` zoom ·
`0` fit-to-width · `Q` quit. (`Ctrl+wheel` is also how Wayland/X11 synthesize
trackpad pinch, since winit only emits `PinchGesture` on macOS/iOS.)

## How it works

`bevy_document_extend` exposes three pieces per format:

- **Types** — e.g. `DocMetadata` / `PdfError`, `DocxMetadata` / `DocxError`,
  `XlsxMetadata` / `XlsxError` in `pdf::types`, `docx::types`, `xlsx::types`.
- **Backends** — a `RasterBackend` trait (`pdf`) / `DocxBackend` (`docx`) /
  `XlsxBackend` (`xlsx`) / `PptxBackend` (`pptx`) with one struct per
  dependency (`HayroBackend`, `ZpdfBackend`, `OfficeDocxBackend`,
  `CalamineOfficeBackend`, `OfficePptxBackend`). `available_backends()`
  and `default_backend()` pick by enabled feature, preferring `hayro` for PDF.
- **Bevy conversion** — `common::ImageBuffer` (raw RGBA + `flattened_on_white`)
  becomes a Bevy `Image` via `Image::from_dynamic` with
  `MAIN_WORLD | RENDER_WORLD` usage and a linear sampler.

Zoom in the viewers is two-level: display zoom resizes the page `Node` from the
existing texture every input tick (GPU scaling, 60fps), while render dpi only
re-rasters after the zoom settles 250ms and drifts past 4% — a pinch never
re-rasterizes ten times mid-gesture.

## Compatibility

| Bevy | bevy_document_extend |
|------|----------------------|
| 0.19 | 0.1.0-alpha.x        |

## License

MIT OR Apache-2.0 (the same dual license as Bevy itself).
