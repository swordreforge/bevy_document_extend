# bevy_document_extend

Pure-Rust document rendering for [Bevy](https://bevyengine.org), backed by
[`hayro`](https://crates.io/crates/hayro) / [`zpdf`](https://crates.io/crates/zpdf)
for PDF and [`office2pdf`](https://crates.io/crates/office2pdf) (+
[`calamine`](https://crates.io/crates/calamine) for XLSX data,
`zip` + `quick-xml` for DOCX/PPTX metadata) for Office.
No C, no FFI, no LibreOffice.

Bevy 0.19 has no built-in document support — `bevy_asset` stops at images/audio,
and high-fidelity Office rendering elsewhere usually means linking LibreOfficeKit.
This crate probes document bytes and rasterizes pages straight into Bevy `Image`
assets, so the whole pipeline stays pure Rust on **native and wasm**.

## Features

- **Zero C/FFI** — no LibreOffice, no `bindgen`, no system toolchain at build time.
- **Feature-gated backends** — PDF / DOCX / XLSX / PPTX are independent Cargo features;
  you only compile what you use.
- **Headless by default** — the default features give you `probe` + `rasterize_*`
  with only `bevy_asset`/`bevy_image` pulled in (no window, no UI, no renderer).
  Opt into the `viewer` feature only for the windowed shell.
- **Probe + rasterize** — `probe` reports page counts and metadata, `to_pdf`
  converts Office docs, `rasterize_page` returns raw RGBA,
  `rasterize_page_to_bevy` returns a ready-to-spawn Bevy `Image`.
- **Bevy-native textures** — linear + 8x anisotropic sampling via
  `ImageSamplerDescriptor`, transparent page areas flattened onto paper white,
  render dpi tracking the window `scale_factor` so HiDPI stays sharp.
- **Viewer shell** — `pdf_viewer` / `docx_viewer` / `xlsx_viewer` / `pptx_viewer`
  share one plugin: browser-style continuous scroll, fit-to-width on open,
  wheel scroll, `Ctrl+wheel`/pinch continuous zoom, debounced re-raster, lazy
  per-page fill (full resident up to 20 pages, ±3 sliding window above that).

## Installation

```toml
[dependencies]
# Default: headless render for PDF + DOCX + XLSX + PPTX
bevy_document_extend = "0.2.0"

# Windowed viewer shell on top of the render path:
# bevy_document_extend = { version = "0.2.0", features = ["viewer"] }

# Or pick backends explicitly (default-features = false):
# bevy_document_extend = { version = "0.2.0", default-features = false, features = ["pdf-hayro"] }
```

| Feature                         | Format | Backend                                  |
|---------------------------------|--------|------------------------------------------|
| `pdf-hayro` (default)           | PDF    | `hayro` rasterizer                       |
| `pdf-zpdf`                      | PDF    | `zpdf` rasterizer                        |
| `docx-office` (default)         | DOCX   | `office2pdf` → PDF → `hayro`             |
| `xlsx-calamine-office2pdf` (default) | XLSX | `calamine` read + `office2pdf` → PDF → `hayro` |
| `pptx-office` (default)         | PPTX   | `office2pdf` → PDF → `hayro`             |
| `render`                        | —      | `bevy_asset` + `bevy_image` + `gestures` only (headless) |
| `viewer`                        | —      | `render` + `bevy_window` + `bevy_ui` + `bevy_ui_render` + `bevy_winit` + `x11`/`wayland` |

All three Office formats share one render chain (`office -> office2pdf ->
pdf -> hayro`), so every `*-office*` feature renders out of the box with no
extra setup. DOCX/PPTX `probe`/`extract_text` (and `calamine` for XLSX cells)
read the OPC package directly (`zip` + `quick-xml`), so metadata never pays a
conversion. Converted PDFs are cached per input, so rendering 180 pages pays
one conversion, not 180.

## Usage

Headless — rasterize a PDF page into a Bevy texture:

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

Office formats follow the same shape under their own modules:

```rust
use bevy_document_extend::{docx, pptx, xlsx};

let meta = docx::probe(&bytes).unwrap();
println!("{} pages, {} paragraphs, {} words", meta.pages, meta.paragraphs, meta.words);
let first = docx::rasterize_page_to_bevy(&bytes, 0, 150).unwrap();
let pdf = docx::to_pdf(&bytes).unwrap();

let meta = xlsx::probe(&bytes).unwrap();
let first = xlsx::rasterize_page_to_bevy(&bytes, 0, 150).unwrap();

let meta = pptx::probe(&bytes).unwrap();
let first = pptx::rasterize_page_to_bevy(&bytes, 0, 150).unwrap();
```

One-line windowed viewer (needs the `viewer` feature):

```rust
use bevy::prelude::*;
use bevy_document_extend::view_pdf;

App::new()
    .add_plugins((DefaultPlugins, view_pdf("docs/report.pdf")))
    .run();
```

`view_docx` / `view_xlsx` / `view_pptx` (and `view_*_with(path, show_hud)`)
work the same way. For full control, compose `DocumentViewerPlugin` +
`DocumentSource` (+ `ViewerOptions::basic` / `without_hud`) directly — see
the `viewer` module docs.

### Viewer examples

```sh
cargo run --release --example pdf_viewer --features viewer,pdf-hayro -- docs/report.pdf [page] [dpi] [--fit | --no-fit] [--no-hud]
cargo run --release --example docx_viewer --features viewer,docx-office -- docs/report.docx [page] [dpi]
cargo run --release --example xlsx_viewer --features viewer,xlsx-calamine -- docs/sheet.xlsx [page] [dpi]
cargo run --release --example pptx_viewer --features viewer,pptx-office -- docs/deck.pptx [page] [dpi]
```

Without a path each example opens its bundled sample under `tests/exp/`.
Omitting `dpi` means fit-to-width; passing `dpi` means fixed zoom
(`--fit` / `--no-fit` override either way).

Controls: `Left`/`Right` page · `Home`/`End` first/last · wheel scroll through
all pages · `Ctrl+wheel`/pinch zoom · `Up`/`Down` zoom · `0` fit-to-width ·
`Q` quit. (`Ctrl+wheel` is also how Wayland/X11 synthesize trackpad pinch,
since winit only emits `PinchGesture` on macOS/iOS.)

## How it works

`bevy_document_extend` exposes three pieces per format:

- **Types** — e.g. `DocMetadata` / `PdfError`, `DocxMetadata` / `DocxError`,
  `XlsxMetadata` / `XlsxError`, `PptxMetadata` / `PptxError` in the per-format
  `types` modules.
- **Backends** — a `RasterBackend` trait (`pdf`) / `DocxBackend` (`docx`) /
  `XlsxBackend` (`xlsx`) / `PptxBackend` (`pptx`) with one struct per
  dependency (`HayroBackend`, `ZpdfBackend`, `OfficeDocxBackend`,
  `CalamineOfficeBackend`, `OfficePptxBackend`). `available_backends()`
  and `default_backend()` pick by enabled feature, preferring `hayro` for PDF.
- **Bevy conversion** — `common::ImageBuffer` (raw RGBA + `flattened_on_white`)
  becomes a Bevy `Image` via `Image::from_dynamic` with
  `MAIN_WORLD | RENDER_WORLD` usage and a linear + anisotropic sampler.

Zoom in the viewers is two-level: display zoom resizes the page `Node`s from
the existing textures every input tick (GPU scaling, 60fps), while render dpi
only re-rasters after the zoom settles 250ms and drifts past 4% — a pinch never
re-rasterizes ten times mid-gesture. Rendering is lazy and bounded: documents
up to 20 pages keep every texture resident; longer ones keep a ±3-page sliding
window around the current page (jump targets fill synchronously, the rest one
page per frame, evicted pages collapse to 1x1 stubs without moving layout).

## Compatibility

| Bevy | bevy_document_extend |
|------|----------------------|
| 0.19 | 0.2.x                |

## License

MIT OR Apache-2.0 (the same dual license as Bevy itself).
