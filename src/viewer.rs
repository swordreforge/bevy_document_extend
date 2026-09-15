//! Scrollable document viewer UI, usable as a Bevy [`Plugin`].
//!
//! Add [`DocumentViewerPlugin`] to your [`App`](bevy::prelude::App) alongside
//! `DefaultPlugins`: the plugin itself is the config — set its fields and add
//! it. It spawns the page viewport, HUD bar, input systems and debounced
//! re-raster wiring for you.
//!
//! Layout: full-window scrollable page image, bottom HUD bar with the probe
//! line plus `←/→ page · wheel/pinch scroll · Ctrl+wheel/pinch zoom · ↑/↓ zoom · 0 fit · Q quit`.
//!
//! Input routing: plain wheel scrolls the viewport; `Ctrl+wheel` zooms (this
//! is how Wayland/X11 synthesize trackpad pinch — the compositor sends wheel
//! events with the Ctrl modifier, since winit only emits `PinchGesture` on
//! macOS/iOS). `PinchGesture` itself is also handled for those platforms.
//!
//! Zoom model: two-level continuous zoom.
//!
//! - Display zoom (`Doc::zoom`) is a float updated immediately on every
//!   input tick; the page Node is resized from the *existing* texture, so
//!   gestures stay smooth at 60fps with pure GPU scaling.
//! - Render resolution (`Doc::rendered_zoom` + integer `dpi`) only refreshes
//!   after the zoom settles ([`RERASTER_DELAY`]) *and* drifts outside
//!   [`RERASTER_BAND`], so a pinch doesn't re-rasterize ten times mid-gesture.
//!   Backends still take integer dpi, so [`render_dpi`] rounds — a future
//!   float-dpi backend only replaces that one function, the UI side is
//!   already float.

use bevy::app::AppExit;
use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy::window::PrimaryWindow;

/// Page-render callback: fresh Bevy [`Image`] on demand; `None` keeps the
/// current page on screen.
type RenderCallback = dyn Fn(usize, u32) -> Option<Image> + Send + Sync;

/// Page bytes plus render closure for the viewer. Insert once before or after
/// adding [`DocumentViewerPlugin`]; the plugin's [`Startup`] system consumes
/// it (registers the first page as an [`Image`] asset, moves the closure into
/// the viewer state).
///
/// Keeping the closure in a [`Resource`] instead of the plugin avoids
/// one-shot interior mutability in `Plugin::build` (`build` only gets `&self`,
/// so a plugin-held `Fn` would need a `Mutex` + `take()` + panic-on-reuse).
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
            })
            .add_systems(Startup, setup_from_source)
            .add_systems(
                Update,
                (navigate, auto_fit, wheel, maybe_reraster, refresh).chain(),
            )
            .add_systems(PostUpdate, reanchor_scroll.after(UiSystems::Layout));
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
}

/// Builds viewer state from [`ViewerConfig`] + [`DocumentSource`].
///
/// Runs on [`Startup`]: derives zoom from the real first-page size,
/// registers it as an [`Image`] asset, and moves the render closure into
/// [`Doc`]. Splitting this out of `Plugin::build` is what lets the plugin
/// stay plain data (no closure, no interior mutability).
fn setup_from_source(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    config: Res<ViewerConfig>,
    mut source: ResMut<DocumentSource>,
) {
    let source = &mut *source;
    let first = std::mem::take(&mut source.first);
    let render = source
        .render
        .take()
        .expect("DocumentSource holds one render closure");
    let native_w = first.width() as f32 * BASE_DPI / config.dpi.max(1) as f32;
    let zoom = if config.fit {
        fit_zoom(ASSUMED_WINDOW_W, native_w)
    } else {
        (config.dpi as f32 / BASE_DPI).clamp(MIN_ZOOM, MAX_ZOOM)
    };
    let dpi = render_dpi(zoom);
    let (w, h) = display_size(first.width(), first.height(), zoom, dpi);
    let (tex_w, tex_h) = (first.width(), first.height());
    let handle = images.add(first);
    let image = handle.clone();
    commands.insert_resource(Doc {
        title: config.title.clone(),
        probe: config.probe.clone(),
        pages: config.pages,
        page: config.page,
        zoom,
        rendered_zoom: zoom,
        zoom_idle: RERASTER_DELAY,
        native_w,
        manual_zoom: !config.fit,
        tex_w,
        tex_h,
        anchor_content: Vec2::ZERO,
        anchor_scroll: Vec2::ZERO,
        anchor_init: false,
        dpi,
        rendered_page: config.page,
        rendered_dpi: dpi,
        image: handle,
        render,
    });
    commands.remove_resource::<DocumentSource>();
    let doc = DocRef {
        title: config.title.clone(),
        probe: config.probe.clone(),
        pages: config.pages,
        page: config.page,
        dpi,
        zoom,
    };
    setup_ui(commands, &doc, image, w, h);
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
    format!(
        "{} — {} · page {}/{} · {}dpi · {:.0}% · ←/→ page · wheel scroll · Ctrl+wheel/pinch zoom · ↑/↓ zoom · 0 fit · Q quit",
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
    page: usize,
    /// Display zoom; 1.0 shows a BASE_DPI render at native pixels. Updated
    /// on every input tick; only drives the page Node size.
    zoom: f32,
    /// Zoom level the current texture was rendered at. Chases `zoom` via
    /// debounced re-raster in [`maybe_reraster`].
    rendered_zoom: f32,
    /// Seconds since the last zoom input; gates the re-raster in
    /// [`maybe_reraster`].
    zoom_idle: f32,
    /// Page width in pixels at BASE_DPI; fit-to-width target is derived from this.
    native_w: f32,
    /// False until the user zooms (or passes an explicit dpi); while false
    /// [`auto_fit`] keeps the page fitted to the window width.
    manual_zoom: bool,
    /// Last rendered texture size, for [`display_size`] without re-render.
    tex_w: u32,
    tex_h: u32,
    /// Validated post-layout `(content, scroll)` snapshot from the previous
    /// frame. [`reanchor_scroll`] diffs the page Node size against
    /// `anchor_content` to tell zoom ticks from scroll-only frames, anchoring
    /// zoom on the viewport-center content point; scroll passes through with
    /// a single fresh clamp.
    anchor_content: Vec2,
    anchor_scroll: Vec2,
    anchor_init: bool,
    /// Committed render dpi. [`maybe_reraster`] updates it from the display
    /// zoom after a settle delay; [`refresh`] re-renders only when
    /// `(page, dpi)` differs from `(rendered_page, rendered_dpi)`, so
    /// per-tick display-zoom changes never trigger a raster.
    dpi: u32,
    rendered_page: usize,
    rendered_dpi: u32,
    image: Handle<Image>,
    render: Box<RenderCallback>,
}

#[derive(Component)]
struct PageImage;

#[derive(Component)]
struct Viewport;

#[derive(Component)]
struct HudText;

fn render_dpi(zoom: f32) -> u32 {
    (BASE_DPI * zoom).round().clamp(36.0, 600.0) as u32
}

/// Re-anchor scroll *after* layout, against fresh geometry.
///
/// Input systems only resize the page Node and bump `ScrollPosition` blindly.
/// This system runs in PostUpdate after [`UiSystems::Layout`] and owns the
/// final scroll value:
/// - content unchanged since last frame → pass the input scroll through,
///   clamped to the fresh max (this is the only clamp in the codebase —
///   the wheel handler does pure addition, no per-tick recompute);
/// - content changed (zoom tick) → keep the content point under the viewport
///   center stable: anchor from last frame's validated `(content, scroll)`
///   snapshot, scale by the real size ratio, re-center, clamp.
///
/// Everything here reads post-layout `ComputedNode`, so there is no
/// stale-size bias — the old code read pre-layout sizes at input time, which
/// is what dragged the zoom center downward as zoom grew.
fn reanchor_scroll(
    mut doc: ResMut<Doc>,
    mut viewport: Query<(&ComputedNode, &mut ScrollPosition), With<Viewport>>,
    mut page_nodes: Query<&mut Node, With<PageImage>>,
) {
    let Ok((computed, mut scroll)) = viewport.single_mut() else {
        return;
    };
    let Ok(mut page) = page_nodes.single_mut() else {
        return;
    };
    let (Val::Px(w), Val::Px(h)) = (page.width, page.height) else {
        return;
    };
    let vis = computed.size * computed.inverse_scale_factor;
    if vis.x <= 1.0 || vis.y <= 1.0 {
        return;
    }
    let content = Vec2::new(w, h);
    // Explicit centering margins instead of margin:Auto. Taffy counts a
    // single-sided auto margin into content_size (logs show content - node
    // == (vis - node)/2), inflating the scroll range with phantom space.
    let mx = ((vis.x - content.x) * 0.5).max(0.0);
    let my = ((vis.y - content.y) * 0.5).max(0.0);
    if !matches!(page.margin.left, Val::Px(x) if (x - mx).abs() <= 0.01) {
        page.margin.left = Val::Px(mx);
        page.margin.right = Val::Px(mx);
    }
    if !matches!(page.margin.top, Val::Px(y) if (y - my).abs() <= 0.01) {
        page.margin.top = Val::Px(my);
        page.margin.bottom = Val::Px(my);
    }
    let total = content + Vec2::new(mx * 2.0, my * 2.0);
    let max = (total - vis).max(Vec2::ZERO);
    let out = if doc.anchor_init && (content - doc.anchor_content).abs().max_element() > 0.5 {
        // Zoom tick: re-center on the viewport-center content point.
        let mut out = Vec2::ZERO;
        for i in 0..2 {
            let v = vis[i].max(1.0);
            let c_new = content[i].max(1.0);
            let c_old = doc.anchor_content[i].max(1.0);
            let p = if c_old <= v {
                c_old * 0.5
            } else {
                doc.anchor_scroll[i] + v * 0.5
            };
            out[i] = (p * (c_new / c_old) - v * 0.5).clamp(0.0, (c_new - v).max(0.0));
        }
        out
    } else {
        // Scroll-only frame (or first frame): keep input scroll, fresh clamp.
        scroll.0.clamp(Vec2::ZERO, max)
    };
    scroll.0 = out;
    doc.anchor_content = content;
    doc.anchor_scroll = out;
    doc.anchor_init = true;
}

fn apply_display_zoom(
    doc: &mut Doc,
    zoom: f32,
    page_nodes: &mut Query<(&mut Node, &mut ImageNode), With<PageImage>>,
) {
    let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    doc.zoom = zoom;
    doc.zoom_idle = 0.0;
    doc.manual_zoom = true;
    let (w, h) = display_size(doc.tex_w, doc.tex_h, doc.zoom, doc.dpi);
    for (mut node, _) in page_nodes {
        node.width = Val::Px(w);
        node.height = Val::Px(h);
    }
}

fn fit_zoom(window_w: f32, native_w: f32) -> f32 {
    ((window_w - VIEWPORT_PADDING) / native_w.max(1.0)).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Display size of the page node for a render of `img_w`x`img_h` pixels.
fn display_size(img_w: u32, img_h: u32, zoom: f32, dpi: u32) -> (f32, f32) {
    let k = zoom * BASE_DPI / dpi.max(1) as f32;
    (img_w as f32 * k, img_h as f32 * k)
}

fn setup_ui(
    mut commands: Commands,
    doc: &(impl HudState + ?Sized),
    image: Handle<Image>,
    w: f32,
    h: f32,
) {
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
                    // NOTE: intentionally NOT centered here. A centered child
                    // that outgrows a scroll container gets its start edge
                    // clipped and unreachable, which made zoom visibly jump
                    // right once the page exceeded the viewport. Centering is
                    // done via auto margins on the page itself instead.
                    // NOTE: no padding on the scroll container either. Padding
                    // persists inside the scrollable area even when overflowing
                    // (extra gray strip at bottom/right, shifted scroll max),
                    // while auto margins collapse to zero. The visual gap is
                    // provided by the margins via `fit_zoom`, not padding.
                    overflow: Overflow::scroll(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.30, 0.33)),
            ))
            .with_children(|viewport| {
                viewport.spawn((
                    PageImage,
                    Node {
                        width: Val::Px(w),
                        height: Val::Px(h),
                        // Auto margins center the page while it fits and
                        // collapse to zero once it overflows, so scroll starts
                        // at the top-left with nothing clipped.
                        margin: UiRect::all(Val::Auto),
                        ..default()
                    },
                    ImageNode::new(image),
                ));
            });
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
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
}

fn navigate(
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut doc: ResMut<Doc>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
) {
    if keys.just_pressed(KeyCode::ArrowRight) && doc.page + 1 < doc.pages {
        doc.page += 1;
    } else if keys.just_pressed(KeyCode::ArrowLeft) && doc.page > 0 {
        doc.page -= 1;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        let zoom = doc.zoom * KEY_ZOOM_STEP;
        apply_display_zoom(&mut doc, zoom, &mut page_nodes);
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        let zoom = doc.zoom / KEY_ZOOM_STEP;
        apply_display_zoom(&mut doc, zoom, &mut page_nodes);
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
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
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
            apply_display_zoom(&mut doc, zoom, &mut page_nodes);
        }
        // Swallow the scroll too: a pinching hand also drifts, and feeding
        // that drift into the viewport fights the zoom.
        scroll_px = Vec2::ZERO;
        lines = Vec2::ZERO;
    } else if pinch != 0.0 {
        // macOS/iOS native gesture (winit never emits this on Linux).
        let zoom = doc.zoom * (pinch * WHEEL_ZOOM_SPEED).exp();
        apply_display_zoom(&mut doc, zoom, &mut page_nodes);
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
fn auto_fit(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut doc: ResMut<Doc>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
) {
    if doc.manual_zoom {
        return;
    }
    if let Ok(window) = windows.single() {
        let zoom = fit_zoom(window.width(), doc.native_w);
        if (zoom - doc.zoom).abs() > 0.0005 {
            apply_display_zoom(&mut doc, zoom, &mut page_nodes);
        }
    }
}

/// Commit a debounced re-raster once the display zoom settles.
///
/// Runs before [`refresh`]: accumulates idle time and, once the zoom has sat
/// still for [`RERASTER_DELAY`] and drifted past [`RERASTER_BAND`], snapshots
/// the display zoom into `dpi`. [`refresh`] then performs the actual render.
fn maybe_reraster(time: Res<Time>, mut doc: ResMut<Doc>) {
    doc.zoom_idle += time.delta_secs();
    if doc.zoom_idle < RERASTER_DELAY {
        return;
    }
    let drift = (doc.zoom - doc.rendered_zoom).abs() / doc.rendered_zoom.max(0.001);
    if drift > RERASTER_BAND {
        doc.dpi = render_dpi(doc.zoom);
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    mut doc: ResMut<Doc>,
    mut images: ResMut<Assets<Image>>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
    mut hud: Query<&mut Text, With<HudText>>,
) {
    let need_raster = doc.page != doc.rendered_page || doc.dpi != doc.rendered_dpi;
    if need_raster
        && let Some(image) = (doc.render)(doc.page, doc.dpi)
        && let Some(mut slot) = images.get_mut(&doc.image)
    {
        (doc.tex_w, doc.tex_h) = (image.width(), image.height());
        doc.rendered_page = doc.page;
        doc.rendered_dpi = doc.dpi;
        doc.rendered_zoom = doc.zoom;
        let (w, h) = display_size(image.width(), image.height(), doc.zoom, doc.dpi);
        *slot = image;
        for (mut node, _) in &mut page_nodes {
            node.width = Val::Px(w);
            node.height = Val::Px(h);
        }
    } else if doc.is_changed() {
        // Display-only change (per-tick zoom Node resize is already done in
        // the input systems); keep the HUD in sync.
    } else {
        return;
    }
    for mut text in &mut hud {
        text.0 = hud_line(&doc);
    }
}
