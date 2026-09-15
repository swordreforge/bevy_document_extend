//! Scrollable document viewer UI, usable as a Bevy [`Plugin`].
//!
//! Add [`DocumentViewerPlugin`] to your [`App`](bevy::prelude::App) alongside
//! `DefaultPlugins`: the plugin itself is the config — set its fields and add
//! it. It spawns the page viewport, HUD bar, input systems and debounced
//! re-raster wiring for you.
//!
//! Layout: full-window vertical column of *all* pages (browser-style
//! continuous scroll: wheel from the first page straight through to the
//! last), optional bottom HUD bar with the probe line plus `[Left]/[Right]
//! page, [Home]/[End] first/last, wheel scroll, Ctrl+wheel/pinch zoom,
//! [Up]/[Down] zoom, 0 fit, Q quit` (see [`DocumentViewerPlugin::show_hud`]).
//! The document still opens fitted to one page width; scrolling moves through
//! pages instead of swapping them.
//!
//! HUD text uses ASCII separators only, so it renders under the default font
//! in any locale.
//!
//! Input routing: plain wheel scrolls the viewport; `Ctrl+wheel` zooms (this
//! is how Wayland/X11 synthesize trackpad pinch — the compositor sends wheel
//! events with the Ctrl modifier, since winit only emits `PinchGesture` on
//! macOS/iOS). `PinchGesture` itself is also handled for those platforms.
//!
//! Zoom model: two-level continuous zoom.
//!
//! - Display zoom (`Doc::zoom`) is a float updated immediately on every
//!   input tick; the page Nodes are resized from the *existing* textures, so
//!   gestures stay smooth at 60fps with pure GPU scaling.
//! - Render resolution (`Doc::rendered_zoom` + integer `dpi`) only refreshes
//!   after the zoom settles ([`RERASTER_DELAY`]) *and* drifts outside
//!   [`RERASTER_BAND`], so a pinch doesn't re-rasterize ten times mid-gesture.
//!   Backends still take integer dpi, so [`render_dpi`] rounds — a future
//!   float-dpi backend only replaces that one function, the UI side is
//!   already float.
//!
//! Render model: two tiers by page count. Documents up to
//! [`FULL_RESIDENT_LIMIT`] (20) pages keep every texture resident — no
//! eviction, no window bookkeeping, just progressive fill; every page stays
//! sharp once rendered. Longer documents switch to a sliding window: only
//! the current page ± [`PREFETCH_RADIUS`] pages hold real textures
//! ([`retained`]); a jump or scroll renders its first missing window page
//! synchronously, then one more per frame until the window is full. Pages
//! outside the window are evicted back to 1x1 white placeholders, so a
//! 180-page document costs ~7 textures instead of 180. Dirty window pages
//! (after a zoom/dpi change) refill the same way, nearest the current page
//! first.

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::UiSystems;
use bevy::window::PrimaryWindow;

/// Page-render callback: fresh Bevy [`Image`] on demand; `None` keeps the
/// current page on screen.
type RenderCallback = dyn Fn(usize, u32) -> Option<Image> + Send + Sync;

/// Page bytes plus render closure for the viewer. Insert once before or after
/// adding [`DocumentViewerPlugin`]; the plugin's [`Startup`] system consumes
/// it (registers one [`Image`] asset per page — the requested page rastered
/// for real, the rest white placeholders that [`refresh`] fills in lazily —
/// and moves the closure into the viewer state).
///
/// This closure is a deliberate extension point, not a shortcut: it is the
/// strategy seam that lets one viewer shell serve any backend (PDF via
/// hayro/zpdf, DOCX/XLSX/PPTX via office2pdf, or a caller's own renderer) without the plugin depending on every format crate. It plays
/// the same role as an `AssetLoader` in `bevy_asset` — per-format loading
/// logic injected at the boundary, rendering itself still done by systems
/// ([`refresh`]) inside the plugin.
#[derive(Resource)]
pub struct DocumentSource {
    pub first: Image,
    render: Option<Box<RenderCallback>>,
}

impl DocumentSource {
    pub fn new(
        first: Image,
        render: impl Fn(usize, u32) -> Option<Image> + Send + Sync + 'static,
    ) -> Self {
        Self {
            first,
            render: Some(Box::new(render)),
        }
    }
}

/// CLI args for the bundled `*_viewer` examples, parsed in one place so the
/// `fit` semantics stay consistent: `path [page] [dpi] [--fit | --no-fit]`.
///
/// Without flags, omitting `dpi` means fit-to-width and passing `dpi` means
/// explicit zoom — `--fit` / `--no-fit` override that default either way.
pub struct ViewerArgs {
    pub path: String,
    pub page: usize,
    pub dpi: u32,
    pub fit: bool,
}

impl ViewerArgs {
    pub fn parse(default_path: &str) -> Self {
        let rest: Vec<String> = std::env::args().skip(1).collect();
        let path = rest
            .first()
            .filter(|s| !s.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| default_path.to_string());
        // Positional slots skip flags, so `viewer file --fit` still resolves
        // page/dpi defaults instead of parsing the flag as a number.
        let positionals: Vec<&String> = rest.iter().filter(|s| !s.starts_with("--")).collect();
        let page = positionals.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let dpi_arg = positionals.get(2).copied();
        let dpi = dpi_arg.and_then(|s| s.parse().ok()).unwrap_or(150);
        let mut fit = dpi_arg.is_none();
        if rest.iter().any(|a| a == "--fit") {
            fit = true;
        }
        if rest.iter().any(|a| a == "--no-fit") {
            fit = false;
        }
        Self {
            path,
            page,
            dpi,
            fit,
        }
    }

    /// True when the raw command line contains `flag` (e.g. `--no-hud`).
    ///
    /// Used for boolean toggles that have no positional slot, mirroring the
    /// `--fit` / `--no-fit` handling in [`ViewerArgs::parse`].
    pub fn has_flag(flag: &str) -> bool {
        std::env::args().skip(1).any(|a| a == flag)
    }

    /// Read `self.path` into memory, panicking with the path on failure.
    ///
    /// Example-oriented helper: viewers always need the raw bytes for
    /// `probe` + first-page raster before the [`DocumentViewerPlugin`] can
    /// be built.
    pub fn read_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.path).unwrap_or_else(|e| panic!("read {}: {e}", self.path))
    }
}

/// Adds the scrollable document viewer UI to an [`App`](bevy::prelude::App).
///
/// The plugin holds plain copyable config; the page closure travels in a
/// [`DocumentSource`] resource, inserted before or after `add_plugins`:
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_document_extend::viewer::{
///     DocumentSource, DocumentViewerPlugin,
/// };
///
/// # fn load_first_page() -> Image { unimplemented!() }
/// # fn render_page(page: usize, dpi: u32) -> Option<Image> { unimplemented!() }
/// App::new()
///     .add_plugins((
///         DefaultPlugins,
///         DocumentViewerPlugin {
///             title: "report.pdf".to_string(),
///             probe: "3 pages".to_string(),
///             pages: 3,
///             page: 0,
///             dpi: 150,
///             fit: true,
///             show_hud: true,
///         },
///     ))
///     .insert_resource(DocumentSource::new(load_first_page(), render_page))
///     .run();
/// ```
pub struct DocumentViewerPlugin {
    pub title: String,
    pub probe: String,
    pub pages: usize,
    pub page: usize,
    pub dpi: u32,
    pub fit: bool,
    /// Show the bottom info bar (title, probe line, page, dpi, zoom, keys).
    /// Defaults to `true`; set `false` for a chromeless embedded page view.
    /// [`ViewerOptions::without_hud`] builds the plugin with it off.
    pub show_hud: bool,
}

impl Plugin for DocumentViewerPlugin {
    fn build(&self, app: &mut App) {
        let page = self.page.min(self.pages.max(1) - 1);
        app.insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.14)))
            .insert_resource(ViewerConfig {
                title: self.title.clone(),
                probe: self.probe.clone(),
                pages: self.pages,
                page,
                dpi: self.dpi,
                fit: self.fit,
                show_hud: self.show_hud,
            })
            .add_systems(Startup, setup_from_source)
            .add_systems(
                Update,
                (navigate, auto_fit, wheel, maybe_reraster, refresh).chain(),
            )
            .add_systems(
                PostUpdate,
                (reanchor_scroll, track_page)
                    .chain()
                    .after(UiSystems::Layout),
            );
    }
}

/// Convenience constructors over [`DocumentViewerPlugin`] so callers don't
/// repeat the full struct literal. Defaults mirror the plugin fields
/// (`fit: true`, `show_hud: true`, `page: 0`); [`ViewerOptions::without_hud`]
/// flips the info bar off for embedded use.
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_document_extend::viewer::ViewerOptions;
///
/// App::new()
///     .add_plugins((DefaultPlugins, ViewerOptions::basic("a.pdf", "1 pages", 1)))
///     .run();
/// ```
pub struct ViewerOptions;

impl ViewerOptions {
    pub fn basic(
        title: impl Into<String>,
        probe: impl Into<String>,
        pages: usize,
    ) -> DocumentViewerPlugin {
        DocumentViewerPlugin {
            title: title.into(),
            probe: probe.into(),
            pages,
            page: 0,
            dpi: 150,
            fit: true,
            show_hud: true,
        }
    }

    pub fn without_hud(
        title: impl Into<String>,
        probe: impl Into<String>,
        pages: usize,
    ) -> DocumentViewerPlugin {
        DocumentViewerPlugin {
            show_hud: false,
            ..Self::basic(title, probe, pages)
        }
    }
}

/// Copyable viewer config, derived from the plugin at build time.
///
/// Kept separate from [`Doc`] (runtime zoom/scroll/render state) so systems
/// read one and mutate the other.
#[derive(Resource)]
struct ViewerConfig {
    title: String,
    probe: String,
    pages: usize,
    page: usize,
    dpi: u32,
    fit: bool,
    show_hud: bool,
}

/// Per-page render slot: one texture handle plus the texel size it was
/// rendered at. `rendered_dpi != Doc::dpi` (or a 1x1 evicted stub whose texel
/// size reads 1x1) means dirty — [`refresh`] fills window pages lazily,
/// nearest the current page first.
struct PageSlot {
    node: Entity,
    handle: Handle<Image>,
    tex_w: u32,
    tex_h: u32,
    rendered_dpi: u32,
}

/// Page count at or below which the viewer keeps every page resident: no
/// eviction, no window bookkeeping — [`refresh`] just fills each page once
/// and it stays sharp. Small documents feel instant; the sliding window
/// ([`PREFETCH_RADIUS`]) only kicks in above this.
const FULL_RESIDENT_LIMIT: usize = 20;

/// Pages on each side of the current page that stay resident on long
/// documents. 3 covers one viewport of context in each direction at fit width.
const PREFETCH_RADIUS: usize = 3;

/// Pages inside the resident window, current page first, then expanding
/// outward — the fill order for jumps and lazy refill on long documents.
/// Short documents (≤ [`FULL_RESIDENT_LIMIT`]) render in plain page order:
/// everything is resident anyway, so filling 0..n needs no bookkeeping.
fn window_order(pages: usize, current: usize) -> Vec<usize> {
    if pages <= FULL_RESIDENT_LIMIT {
        return (0..pages).collect();
    }
    let mut order = Vec::new();
    if pages == 0 {
        return order;
    }
    let current = current.min(pages - 1);
    order.push(current);
    for d in 1..=PREFETCH_RADIUS {
        if current >= d {
            order.push(current - d);
        }
        if current + d < pages {
            order.push(current + d);
        }
    }
    order
}

/// True when `page` belongs in the window around `current`. Short documents
/// retain everything — no eviction below [`FULL_RESIDENT_LIMIT`].
fn retained(pages: usize, current: usize, page: usize) -> bool {
    if pages <= FULL_RESIDENT_LIMIT {
        return true;
    }
    page < pages && page.abs_diff(current.min(pages.saturating_sub(1))) <= PREFETCH_RADIUS
}

/// Builds viewer state from [`ViewerConfig`] + [`DocumentSource`].
///
/// Runs on [`Startup`]: derives zoom from the real requested-page size,
/// registers one [`Image`] asset per page (the start window rendered for
/// real, the rest 1x1 evicted stubs), spawns the column UI, and moves the
/// render closure into [`Doc`]. Splitting this out of `Plugin::build` is
/// what lets the plugin stay plain data (no closure, no interior mutability).
#[allow(clippy::too_many_arguments)]
fn setup_from_source(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    config: Res<ViewerConfig>,
    mut source: ResMut<DocumentSource>,
) {
    let source = &mut *source;
    let first = std::mem::take(&mut source.first);
    let render = source
        .render
        .take()
        .expect("DocumentSource holds one render closure");
    let (fw, fh) = (first.width().max(1), first.height().max(1));
    let scale = windows
        .single()
        .map(|w| w.scale_factor().max(1.0))
        .unwrap_or(1.0);
    let native_w = fw as f32 * BASE_DPI / config.dpi.max(1) as f32;
    let zoom = if config.fit {
        fit_zoom(ASSUMED_WINDOW_W, native_w)
    } else {
        (config.dpi as f32 / BASE_DPI).clamp(MIN_ZOOM, MAX_ZOOM)
    };
    let dpi = (render_dpi(zoom) as f32 * scale).round().clamp(36.0, 600.0) as u32;
    let pages = config.pages.max(1);
    let start = config.page.min(pages - 1);
    // Resident window first: the start image arrives pre-rendered in
    // `DocumentSource`, so keep it and synchronously fill the rest of the
    // start window (at most 2*RADIUS more backend renders). Everything
    // outside the window gets a 1x1 evicted stub on the GPU — but the slot
    // keeps the `fw`x`fh` *estimate* for layout, so the column total is
    // stable from the first frame instead of growing for minutes while lazy
    // fills land (that growth is what used to drag scroll off the top).
    let mut first = Some(first);
    let mut slots = Vec::with_capacity(pages);
    for i in 0..pages {
        if i == start {
            let img = first.take().expect("requested page image");
            let (tw, th) = (img.width().max(1), img.height().max(1));
            let handle = images.add(img);
            slots.push(PageSlot {
                node: Entity::PLACEHOLDER,
                handle,
                tex_w: tw,
                tex_h: th,
                rendered_dpi: dpi,
            });
        } else {
            let handle = images.add(evicted_image());
            slots.push(PageSlot {
                node: Entity::PLACEHOLDER,
                handle,
                tex_w: fw,
                tex_h: fh,
                // 0 is outside the 36..=600 dpi range, so the slot reads as
                // dirty until its window turn comes.
                rendered_dpi: 0,
            });
        }
    }
    for &i in &window_order(pages, start) {
        if i == start {
            continue;
        }
        render_slot(&render, &mut images, &mut slots[i], i, dpi);
    }
    let sizes: Vec<(f32, f32)> = slots
        .iter()
        .map(|s| display_size(s.tex_w, s.tex_h, zoom, dpi))
        .collect();
    let handles: Vec<Handle<Image>> = slots.iter().map(|s| s.handle.clone()).collect();
    let doc = DocRef {
        title: config.title.clone(),
        probe: config.probe.clone(),
        pages,
        page: start,
        dpi,
        zoom,
    };
    let entities = setup_ui(&mut commands, &doc, &handles, &sizes, config.show_hud);
    for (slot, entity) in slots.iter_mut().zip(entities) {
        slot.node = entity;
    }
    commands.insert_resource(Doc {
        title: config.title.clone(),
        probe: config.probe.clone(),
        pages,
        page: start,
        zoom,
        rendered_zoom: zoom,
        zoom_idle: RERASTER_DELAY,
        native_w,
        manual_zoom: !config.fit,
        anchor_total: Vec2::ZERO,
        anchor_scroll: Vec2::ZERO,
        anchor_init: false,
        dpi,
        slots,
        render,
    });
    commands.remove_resource::<DocumentSource>();
}

/// 1x1 evicted stub: frees the texture while keeping the handle alive. The
/// slot's `tex_w`/`tex_h` estimate (first-page size) keeps driving layout,
/// so eviction never collapses the column — it only swaps GPU bytes for a
/// single white pixel.
fn evicted_image() -> Image {
    blank_image(1, 1)
}

/// Opaque paper-white texture. Used for evicted stubs (1x1) only — real
/// pages always render through the backend closure.
fn blank_image(w: u32, h: u32) -> Image {
    Image::new_fill(
        Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Minimal snapshot for HUD text — implemented for both the [`setup_ui`]
/// bootstrap ([`DocRef`]) and the runtime [`Doc`], so one format string
/// serves initial spawn and per-frame [`refresh`] updates.
trait HudState {
    fn title(&self) -> &str;
    fn probe(&self) -> &str;
    fn page(&self) -> usize;
    fn pages(&self) -> usize;
    fn dpi(&self) -> u32;
    fn zoom(&self) -> f32;
}

struct DocRef {
    title: String,
    probe: String,
    pages: usize,
    page: usize,
    dpi: u32,
    zoom: f32,
}

impl HudState for DocRef {
    fn title(&self) -> &str {
        &self.title
    }
    fn probe(&self) -> &str {
        &self.probe
    }
    fn page(&self) -> usize {
        self.page
    }
    fn pages(&self) -> usize {
        self.pages
    }
    fn dpi(&self) -> u32 {
        self.dpi
    }
    fn zoom(&self) -> f32 {
        self.zoom
    }
}

impl HudState for Doc {
    fn title(&self) -> &str {
        &self.title
    }
    fn probe(&self) -> &str {
        &self.probe
    }
    fn page(&self) -> usize {
        self.page
    }
    fn pages(&self) -> usize {
        self.pages
    }
    fn dpi(&self) -> u32 {
        self.dpi
    }
    fn zoom(&self) -> f32 {
        self.zoom
    }
}

impl<T: HudState + ?Sized> HudState for &T {
    fn title(&self) -> &str {
        (**self).title()
    }
    fn probe(&self) -> &str {
        (**self).probe()
    }
    fn page(&self) -> usize {
        (**self).page()
    }
    fn pages(&self) -> usize {
        (**self).pages()
    }
    fn dpi(&self) -> u32 {
        (**self).dpi()
    }
    fn zoom(&self) -> f32 {
        (**self).zoom()
    }
}

impl HudState for Res<'_, Doc> {
    fn title(&self) -> &str {
        (**self).title()
    }
    fn probe(&self) -> &str {
        (**self).probe()
    }
    fn page(&self) -> usize {
        (**self).page()
    }
    fn pages(&self) -> usize {
        (**self).pages()
    }
    fn dpi(&self) -> u32 {
        (**self).dpi()
    }
    fn zoom(&self) -> f32 {
        (**self).zoom()
    }
}

impl HudState for ResMut<'_, Doc> {
    fn title(&self) -> &str {
        (**self).title()
    }
    fn probe(&self) -> &str {
        (**self).probe()
    }
    fn page(&self) -> usize {
        (**self).page()
    }
    fn pages(&self) -> usize {
        (**self).pages()
    }
    fn dpi(&self) -> u32 {
        (**self).dpi()
    }
    fn zoom(&self) -> f32 {
        (**self).zoom()
    }
}

fn hud_line(doc: &(impl HudState + ?Sized)) -> String {
    // ASCII separators only: the HUD text must render under the default font
    // in any locale. Non-ASCII separators (middle dot, em dash, CJK arrows)
    // fall back to tofu boxes when the system font lacks those glyphs.
    format!(
        "{}, {}, page {}/{}, {}dpi, {:.0}%, [Left]/[Right] page, [Home]/[End] first/last, wheel scroll, Ctrl+wheel/pinch zoom, [Up]/[Down] zoom, 0 fit, Q quit",
        doc.title(),
        doc.probe(),
        doc.page() + 1,
        doc.pages(),
        doc.dpi(),
        doc.zoom() * 100.0,
    )
}

/// Render resolution at zoom == 1.0.
const BASE_DPI: f32 = 150.0;
const MIN_ZOOM: f32 = 0.2;
const MAX_ZOOM: f32 = 4.0;
const KEY_ZOOM_STEP: f32 = 1.25;
/// Trackpad pinch arrives as Ctrl+wheel with tiny per-tick deltas, so the
/// zoom gain must be much higher than for stepped mouse wheels.
const WHEEL_ZOOM_SPEED: f32 = 0.35;
const LINE_SCROLL_PX: f32 = 32.0;
/// Vertical gap between consecutive pages in the scroll column.
const PAGE_GAP: f32 = 16.0;
/// Zoom must sit still this long before a re-raster fires, so mid-gesture
/// ticks only GPU-scale the existing texture.
const RERASTER_DELAY: f32 = 0.25;
/// Re-raster only when display zoom drifts this far (relative) from the
/// rendered zoom — filters out sub-pixel settle noise.
const RERASTER_BAND: f32 = 0.04;
/// Assumed window width before the first real size is observed; the
/// [`auto_fit`] system corrects it on the first frame anyway.
const ASSUMED_WINDOW_W: f32 = 1280.0;
const VIEWPORT_PADDING: f32 = 24.0;

#[derive(Resource)]
struct Doc {
    title: String,
    probe: String,
    pages: usize,
    /// Current page: set by jumps ([`jump_to_page`]) and kept live while
    /// scrolling by [`track_page`]; drives the HUD readout and the
    /// nearest-first order in [`refresh`].
    page: usize,
    /// Display zoom; 1.0 shows a BASE_DPI render at native pixels. Updated
    /// on every input tick; only drives the page Node sizes.
    zoom: f32,
    /// Zoom level every slot is rendered at. Chases `zoom` via debounced
    /// re-raster in [`maybe_reraster`]; only re-armed once *all* slots are
    /// clean, so progressive per-page fills don't confuse the drift check.
    rendered_zoom: f32,
    /// Seconds since the last zoom input; gates the re-raster in
    /// [`maybe_reraster`].
    zoom_idle: f32,
    /// Page width in pixels at BASE_DPI; fit-to-width target is derived from this.
    native_w: f32,
    /// False until the user zooms (or passes an explicit dpi); while false
    /// [`auto_fit`] keeps the page fitted to the window width.
    manual_zoom: bool,
    /// Validated post-layout total-column-size + scroll snapshot from the
    /// previous frame. [`reanchor_scroll`] diffs the column total against
    /// `anchor_total` to tell zoom ticks from scroll-only frames, anchoring
    /// zoom on the viewport-center content point; scroll passes through with
    /// a single fresh clamp.
    anchor_total: Vec2,
    anchor_scroll: Vec2,
    anchor_init: bool,
    /// Committed render dpi. [`maybe_reraster`] updates it from the display
    /// zoom after a settle delay; [`refresh`] re-renders dirty slots only
    /// (one per frame, nearest first), so per-tick display-zoom changes
    /// never trigger a raster.
    dpi: u32,
    slots: Vec<PageSlot>,
    render: Box<RenderCallback>,
}

#[derive(Component)]
struct PageImage(usize);

#[derive(Component)]
struct Viewport;

#[derive(Component)]
struct HudText;

fn render_dpi(zoom: f32) -> u32 {
    (BASE_DPI * zoom).round().clamp(36.0, 600.0) as u32
}

/// Render one slot through the backend closure into its asset.
///
/// Returns true on success; on failure (`None`) the placeholder stays and
/// the slot keeps reading as dirty, so a later frame retries.
fn render_slot(
    render: &RenderCallback,
    images: &mut Assets<Image>,
    slot: &mut PageSlot,
    page: usize,
    dpi: u32,
) -> bool {
    if let Some(image) = render(page, dpi)
        && let Some(mut dest) = images.get_mut(&slot.handle)
    {
        slot.tex_w = image.width().max(1);
        slot.tex_h = image.height().max(1);
        slot.rendered_dpi = dpi;
        *dest = image;
        return true;
    }
    false
}

/// Re-anchor scroll *after* layout, against fresh geometry.
///
/// Input systems only bump `ScrollPosition` blindly and [`refresh`] only
/// resizes page Nodes. This system runs in PostUpdate after
/// [`UiSystems::Layout`] and owns the final scroll value:
/// - column total unchanged since last frame → pass the input scroll through,
///   clamped to the fresh max (this is the only clamp in the codebase —
///   the wheel handler does pure addition, no per-tick recompute);
/// - column total changed (zoom tick or fresh page texture) → keep the content
///   point under the viewport center stable: anchor from last frame's
///   validated `(total, scroll)` snapshot, scale by the real size ratio,
///   re-center, clamp.
///
/// Everything here reads post-layout `ComputedNode`, so there is no
/// stale-size bias — the old code read pre-layout sizes at input time, which
/// is what dragged the zoom center downward as zoom grew.
fn reanchor_scroll(
    mut doc: ResMut<Doc>,
    mut viewport: Query<(&ComputedNode, &mut ScrollPosition), With<Viewport>>,
    mut page_nodes: Query<(&mut Node, &PageImage)>,
) {
    let Ok((computed, mut scroll)) = viewport.single_mut() else {
        return;
    };
    let vis = computed.size * computed.inverse_scale_factor;
    if vis.x <= 1.0 || vis.y <= 1.0 {
        return;
    }
    let sizes = page_display_sizes(&doc);
    if sizes.len() != doc.slots.len() || sizes.is_empty() {
        return;
    }
    let total = column_total(&sizes);
    // Explicit centering margins instead of margin:Auto. Taffy counts a
    // single-sided auto margin into content_size (logs show content - node
    // == (vis - node)/2), inflating the scroll range with phantom space.
    for (mut node, idx) in &mut page_nodes {
        let Some(&(w, _)) = sizes.get(idx.0) else {
            continue;
        };
        let mx = ((vis.x - w) * 0.5).max(0.0);
        if !matches!(node.margin.left, Val::Px(x) if (x - mx).abs() <= 0.01) {
            node.margin.left = Val::Px(mx);
            node.margin.right = Val::Px(mx);
        }
        if !matches!(node.margin.top, Val::Px(x) if x.abs() <= 0.01) {
            node.margin.top = Val::Px(0.0);
            node.margin.bottom = Val::Px(0.0);
        }
    }
    // When the whole column fits, pad the first page down so it sits
    // vertically centered instead of glued to the top.
    let pad = ((vis.y - total.y) * 0.5).max(0.0);
    for (mut node, idx) in &mut page_nodes {
        if idx.0 == 0 && !matches!(node.margin.top, Val::Px(x) if (x - pad).abs() <= 0.01) {
            node.margin.top = Val::Px(pad);
        }
    }
    let max = (total - vis).max(Vec2::ZERO);
    let out = if !doc.anchor_init && doc.page > 0 {
        // Opening on a requested start page (`viewer file.pdf 2`): jump
        // straight there on the first frame instead of opening at the top.
        Vec2::new(0.0, page_offset_y(&sizes, doc.page).clamp(0.0, max.y))
    } else if total.y > doc.anchor_total.y + 0.5 && doc.anchor_init && doc.anchor_scroll.y <= 0.5 {
        // Fresh page textures landing below while pinned to the top (the
        // 180-page case: one lazy render per frame keeps growing the column
        // for minutes). Hold scroll at 0 instead of letting the center-anchor
        // math below convert real-size growth into a downward drift.
        Vec2::new(scroll.0.x.clamp(0.0, max.x), 0.0)
    } else if doc.anchor_init && (total - doc.anchor_total).abs().max_element() > 0.5 {
        // Zoom tick (or a fresh page texture landing): re-center on the
        // viewport-center content point — except when pinned to an edge.
        // Without the edge stickiness, opening drifts down a few pixels:
        // the first `auto_fit` correction grows the column while scroll is
        // still 0, and center-anchoring pushes scroll to `v*0.5*(ratio-1)`.
        let mut out = Vec2::ZERO;
        for i in 0..2 {
            let v = vis[i].max(1.0);
            let c_new = total[i].max(1.0);
            let c_old = doc.anchor_total[i].max(1.0);
            let prev_max = (c_old - v).max(0.0);
            if doc.anchor_scroll[i] <= 0.5 {
                out[i] = 0.0;
            } else if doc.anchor_scroll[i] >= prev_max - 0.5 {
                out[i] = (c_new - v).max(0.0);
            } else {
                let p = doc.anchor_scroll[i] + v * 0.5;
                out[i] = (p * (c_new / c_old) - v * 0.5).clamp(0.0, (c_new - v).max(0.0));
            }
        }
        out
    } else {
        // Scroll-only frame (or first frame): keep input scroll, fresh clamp.
        scroll.0.clamp(Vec2::ZERO, max)
    };
    scroll.0 = out;
    doc.anchor_total = total;
    doc.anchor_scroll = out;
    doc.anchor_init = true;
}

fn apply_display_zoom(doc: &mut Doc, zoom: f32) {
    doc.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    doc.zoom_idle = 0.0;
    doc.manual_zoom = true;
    // Node sizes are reconciled in `refresh` (same frame, later in the
    // chain), so zoom ticks stay pure state updates here.
}

fn fit_zoom(window_w: f32, native_w: f32) -> f32 {
    ((window_w - VIEWPORT_PADDING) / native_w.max(1.0)).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Display size of the page node for a render of `img_w`x`img_h` pixels.
fn display_size(img_w: u32, img_h: u32, zoom: f32, dpi: u32) -> (f32, f32) {
    let k = zoom * BASE_DPI / dpi.max(1) as f32;
    (img_w as f32 * k, img_h as f32 * k)
}

/// Display sizes of every slot at the current zoom — the single source of
/// truth for layout, scroll offsets and the zoom anchor.
fn page_display_sizes(doc: &Doc) -> Vec<(f32, f32)> {
    doc.slots
        .iter()
        .map(|s| display_size(s.tex_w, s.tex_h, doc.zoom, doc.dpi))
        .collect()
}

/// Total column size: widest page across, heights plus gaps down.
fn column_total(sizes: &[(f32, f32)]) -> Vec2 {
    let w = sizes.iter().map(|(w, _)| *w).fold(0.0, f32::max);
    let h = sizes.iter().map(|(_, h)| *h).sum::<f32>()
        + PAGE_GAP * sizes.len().saturating_sub(1) as f32;
    Vec2::new(w, h)
}

/// Y offset of the top edge of `page` inside the column.
fn page_offset_y(sizes: &[(f32, f32)], page: usize) -> f32 {
    let page = page.min(sizes.len().saturating_sub(1));
    sizes[..page].iter().map(|(_, h)| *h).sum::<f32>() + PAGE_GAP * page as f32
}

fn setup_ui(
    commands: &mut Commands,
    doc: &(impl HudState + ?Sized),
    handles: &[Handle<Image>],
    sizes: &[(f32, f32)],
    show_hud: bool,
) -> Vec<Entity> {
    let mut entities = Vec::with_capacity(handles.len());
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.14)),
        ))
        .with_children(|root| {
            root.spawn((
                Viewport,
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(PAGE_GAP),
                    // NOTE: no centering or padding on the scroll container.
                    // A centered child that outgrows it gets its start edge
                    // clipped and unreachable; padding persists inside the
                    // scrollable area as dead strips. Per-page explicit
                    // margins in `reanchor_scroll` do the centering instead.
                    overflow: Overflow::scroll(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.30, 0.33)),
            ))
            .with_children(|viewport| {
                for (i, (handle, (w, h))) in handles.iter().zip(sizes).enumerate() {
                    let entity = viewport
                        .spawn((
                            PageImage(i),
                            Node {
                                width: Val::Px(*w),
                                height: Val::Px(*h),
                                ..default()
                            },
                            ImageNode::new(handle.clone()),
                        ))
                        .id();
                    entities.push(entity);
                }
            });
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    display: if show_hud {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
            ))
            .with_children(|hud| {
                hud.spawn((
                    HudText,
                    Text::new(hud_line(doc)),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::WHITE),
                ));
            });
        });
    entities
}

/// Jump the viewport to `target`: on long documents fill the target window
/// synchronously (same instant feedback the old single-page viewer had on
/// every page turn) and evict the pages that fell out; on short documents
/// (everything resident) just render the target if still dirty. Then scroll
/// the target's top edge into view. Wheel scrolling into a filled window
/// needs no render at all — the prefetch is already resident.
fn jump_to_page(
    doc: &mut Doc,
    images: &mut Assets<Image>,
    scroll: &mut Query<&mut ScrollPosition, With<Viewport>>,
    target: usize,
) {
    let target = target.min(doc.pages.max(1) - 1);
    doc.page = target;
    let dpi = doc.dpi;
    evict_outside_window(doc, images, target);
    for &i in &window_order(doc.pages, target) {
        render_slot(&doc.render, images, &mut doc.slots[i], i, dpi);
    }
    let sizes = page_display_sizes(doc);
    if let Ok(mut pos) = scroll.single_mut() {
        pos.0.y = page_offset_y(&sizes, target);
    }
}

/// Swap every texture outside the window around `current` for a 1x1 stub.
///
/// Keeps `tex_w`/`tex_h` estimates (and `rendered_dpi`, so the slot still
/// reads dirty at its true size later) — only GPU bytes are freed, layout
/// never moves. Skips slots already stubbed so settled frames do zero
/// asset writes.
fn evict_outside_window(doc: &mut Doc, images: &mut Assets<Image>, current: usize) {
    let pages = doc.pages;
    for (i, slot) in doc.slots.iter_mut().enumerate() {
        if retained(pages, current, i) {
            continue;
        }
        if slot.tex_w <= 1 && slot.tex_h <= 1 {
            continue;
        }
        if let Some(mut dest) = images.get_mut(&slot.handle) {
            *dest = evicted_image();
            slot.rendered_dpi = 0;
        }
    }
}

fn navigate(
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut doc: ResMut<Doc>,
    mut images: ResMut<Assets<Image>>,
    mut scroll: Query<&mut ScrollPosition, With<Viewport>>,
) {
    if keys.just_pressed(KeyCode::ArrowRight) && doc.page + 1 < doc.pages {
        let target = doc.page + 1;
        jump_to_page(&mut doc, &mut images, &mut scroll, target);
    } else if keys.just_pressed(KeyCode::ArrowLeft) && doc.page > 0 {
        let target = doc.page - 1;
        jump_to_page(&mut doc, &mut images, &mut scroll, target);
    } else if keys.just_pressed(KeyCode::Home) {
        jump_to_page(&mut doc, &mut images, &mut scroll, 0);
    } else if keys.just_pressed(KeyCode::End) {
        let last = doc.pages.max(1) - 1;
        jump_to_page(&mut doc, &mut images, &mut scroll, last);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        let zoom = doc.zoom * KEY_ZOOM_STEP;
        apply_display_zoom(&mut doc, zoom);
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        let zoom = doc.zoom / KEY_ZOOM_STEP;
        apply_display_zoom(&mut doc, zoom);
    } else if keys.just_pressed(KeyCode::Digit0) {
        doc.manual_zoom = false;
    } else if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
    }
}

fn wheel(
    mut wheels: MessageReader<MouseWheel>,
    mut pinches: MessageReader<PinchGesture>,
    keys: Res<ButtonInput<KeyCode>>,
    mut doc: ResMut<Doc>,
    mut scroll: Query<&mut ScrollPosition, With<Viewport>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let mut lines = Vec2::ZERO;
    let mut scroll_px = Vec2::ZERO;
    let mut saw_pixel_wheel = false;
    for wheel in wheels.read() {
        match wheel.unit {
            MouseScrollUnit::Line => lines += Vec2::new(wheel.x, wheel.y),
            // Trackpad scroll/pinch arrives as many tiny Pixel ticks; keep
            // them out of the Line accumulator so zoom math stays in one unit.
            MouseScrollUnit::Pixel => {
                scroll_px += Vec2::new(wheel.x, wheel.y);
                saw_pixel_wheel = true;
            }
        }
    }
    let pinch: f32 = pinches.read().map(|p| p.0).sum();
    if ctrl {
        // On Wayland/X11 a trackpad pinch is synthesized as Ctrl+Pixel-wheel:
        // Pixel deltas (~a few px per tick) become the zoom energy, Line
        // deltas (real stepped wheels held with Ctrl) stay step-like.
        let pixel_part = if saw_pixel_wheel {
            scroll_px.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        } else {
            0.0
        };
        let total = lines.y + pixel_part + pinch;
        if total != 0.0 {
            let zoom = doc.zoom * (total * WHEEL_ZOOM_SPEED).exp();
            apply_display_zoom(&mut doc, zoom);
        }
        // Swallow the scroll too: a pinching hand also drifts, and feeding
        // that drift into the viewport fights the zoom.
        scroll_px = Vec2::ZERO;
        lines = Vec2::ZERO;
    } else if pinch != 0.0 {
        // macOS/iOS native gesture (winit never emits this on Linux).
        let zoom = doc.zoom * (pinch * WHEEL_ZOOM_SPEED).exp();
        apply_display_zoom(&mut doc, zoom);
    }
    if lines != Vec2::ZERO || scroll_px != Vec2::ZERO {
        // Wheel-up (positive y) shows earlier content: move the viewport up.
        // Pure addition here — the single fresh clamp lives in
        // [`reanchor_scroll`] (PostUpdate, after layout), so scrolling no
        // longer recomputes anything per tick.
        if let Ok(mut pos) = scroll.single_mut() {
            pos.0 -= scroll_px + lines * LINE_SCROLL_PX;
        }
    }
}

/// Keep fit-to-width live while the user hasn't taken over zoom.
///
/// The render dpi tracks the *physical* pixel density (`scale_factor`), so a
/// HiDPI/retina display renders at 2x texels instead of GPU-upscaling a 1x
/// texture — that upscale blur is the main "slightly fuzzy vs browser" gap.
fn auto_fit(windows: Query<&Window, With<PrimaryWindow>>, mut doc: ResMut<Doc>) {
    if doc.manual_zoom {
        return;
    }
    if let Ok(window) = windows.single() {
        let zoom = fit_zoom(window.width(), doc.native_w);
        let scale = window.scale_factor().max(1.0);
        let dpi = (render_dpi(zoom) as f32 * scale).round().clamp(36.0, 600.0) as u32;
        if dpi != doc.dpi {
            doc.dpi = dpi;
        }
        if (zoom - doc.zoom).abs() > 0.0005 {
            apply_display_zoom(&mut doc, zoom);
        }
    }
}

/// Commit a debounced re-raster once the display zoom settles.
///
/// Runs before [`refresh`]: accumulates idle time and, once the zoom has sat
/// still for [`RERASTER_DELAY`] and drifted past [`RERASTER_BAND`], snapshots
/// the display zoom into `dpi`. [`refresh`] then performs the actual render.
/// Like [`auto_fit`], dpi is scaled by the window's physical pixel ratio so
/// the texture matches the display instead of being GPU-upscaled.
fn maybe_reraster(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut doc: ResMut<Doc>,
) {
    doc.zoom_idle += time.delta_secs();
    if doc.zoom_idle < RERASTER_DELAY {
        return;
    }
    let drift = (doc.zoom - doc.rendered_zoom).abs() / doc.rendered_zoom.max(0.001);
    if drift > RERASTER_BAND {
        let scale = windows
            .single()
            .map(|w| w.scale_factor().max(1.0))
            .unwrap_or(1.0);
        doc.dpi = (render_dpi(doc.zoom) as f32 * scale)
            .round()
            .clamp(36.0, 600.0) as u32;
    }
}

fn refresh(
    mut doc: ResMut<Doc>,
    mut images: ResMut<Assets<Image>>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode, &PageImage)>,
    mut hud: Query<&mut Text, With<HudText>>,
) {
    // Reconcile every Node with the current zoom/dpi. Guarded writes only,
    // so a settled frame touches nothing and skips layout invalidation.
    let sizes = page_display_sizes(&doc);
    for (mut node, _, idx) in &mut page_nodes {
        if let Some(&(w, h)) = sizes.get(idx.0) {
            if !matches!(node.width, Val::Px(x) if (x - w).abs() <= 0.01) {
                node.width = Val::Px(w);
            }
            if !matches!(node.height, Val::Px(x) if (x - h).abs() <= 0.01) {
                node.height = Val::Px(h);
            }
        }
    }
    // Page maintenance, two tiers. Short documents (<= FULL_RESIDENT_LIMIT):
    // no eviction, just fill each page once in order — everything stays
    // sharp. Long documents: evict what scrolled out of the ±3 window, then
    // fill one missing (or dirty, after a zoom/dpi change) window page per
    // frame, current page first. One backend render per frame keeps scrolling
    // at 60fps; a zoom settle sharpens the window progressively instead of
    // stalling. Pages outside the window never render — that is the whole
    // point of the 180-page fix: memory stays at ~7 textures, not 180.
    let doc_mut = &mut *doc;
    evict_outside_window(doc_mut, &mut images, doc_mut.page);
    let order = window_order(doc_mut.pages, doc_mut.page);
    let mut all_clean = true;
    for i in order {
        if doc_mut.slots[i].rendered_dpi != doc_mut.dpi {
            all_clean = false;
            let dpi = doc_mut.dpi;
            render_slot(&doc_mut.render, &mut images, &mut doc_mut.slots[i], i, dpi);
            break;
        }
    }
    if all_clean {
        doc_mut.rendered_zoom = doc_mut.zoom;
    }
    // Content-compare the HUD line: `doc` mutates most frames during
    // progressive fills, and rewriting `Text` unconditionally would redo
    // text layout every frame for an identical string.
    let line = hud_line(&doc);
    for mut text in &mut hud {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

/// Track the current page from the scroll position.
///
/// Runs in PostUpdate after [`reanchor_scroll`], so `scroll` is the validated
/// value: whichever page holds the viewport center is current. Keeps the HUD
/// readout and [`refresh`]'s nearest-first order live while wheel-scrolling
/// through the column.
fn track_page(
    mut doc: ResMut<Doc>,
    viewport: Query<(&ComputedNode, &ScrollPosition), With<Viewport>>,
) {
    let Ok((computed, scroll)) = viewport.single() else {
        return;
    };
    let vis = computed.size * computed.inverse_scale_factor;
    if vis.y <= 1.0 {
        return;
    }
    let sizes = page_display_sizes(&doc);
    if sizes.is_empty() {
        return;
    }
    let center = scroll.0.y + vis.y * 0.5;
    let mut acc = 0.0;
    let mut current = 0;
    for (i, (_, h)) in sizes.iter().enumerate() {
        if center >= acc {
            current = i;
        }
        acc += h + PAGE_GAP;
    }
    doc.page = current.min(doc.pages.max(1) - 1);
}
