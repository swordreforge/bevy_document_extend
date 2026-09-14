//! Shared Bevy UI shell for the `*_viewer` examples.
//!
//! Each format viewer resolves its CLI args (path/page/dpi), then calls
//! [`run`] with a pre-rasterized first page plus a `render` closure so the
//! UI only swaps the page [`Image`] asset on navigation — no re-parse of the
//! format is needed per key press.
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
//!
//! This file is a shared module, not a runnable example: every viewer does
//! `#[path = "viewer_common/mod.rs"] mod viewer_common;` to include it.

use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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
    /// Committed render dpi. [`maybe_reraster`] updates it from the display
    /// zoom after a settle delay; [`refresh`] re-renders only when
    /// `(page, dpi)` differs from `(rendered_page, rendered_dpi)`, so
    /// per-tick display-zoom changes never trigger a raster.
    dpi: u32,
    rendered_page: usize,
    rendered_dpi: u32,
    image: Handle<Image>,
    render: Box<dyn Fn(usize, u32) -> Option<Image> + Send + Sync>,
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

/// Display-side zoom update: resize the page Node from the existing texture
/// immediately (the expensive re-raster happens later in [`maybe_reraster`]),
/// keeping the content point under the viewport center stable.
///
/// Without the scroll compensation, an overflowing page stays top-left
/// anchored (scroll 0,0) while zooming, so growth appears to extend
/// right/down only — "zooms from the left edge". Re-anchoring per axis:
/// take the content point under the viewport center before the resize
/// (content center while it fits, `scroll + center` once overflowing),
/// scale it by the size ratio, and re-center the viewport on it.
fn apply_display_zoom(
    doc: &mut Doc,
    zoom: f32,
    page_nodes: &mut Query<(&mut Node, &mut ImageNode), With<PageImage>>,
    viewport: &mut Query<(&ComputedNode, &mut ScrollPosition), With<Viewport>>,
) {
    let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let old = display_size(doc.tex_w, doc.tex_h, doc.zoom, doc.dpi);
    let new = display_size(doc.tex_w, doc.tex_h, zoom, doc.dpi);
    doc.zoom = zoom;
    doc.zoom_idle = 0.0;
    doc.manual_zoom = true;
    for (mut node, _) in page_nodes {
        node.width = Val::Px(new.0);
        node.height = Val::Px(new.1);
    }
    if let Ok((computed, mut scroll)) = viewport.single_mut() {
        let vis = computed.size * computed.inverse_scale_factor - Vec2::splat(VIEWPORT_PADDING);
        let old = Vec2::new(old.0, old.1);
        let new = Vec2::new(new.0, new.1);
        let mut s = scroll.0;
        for i in 0..2 {
            let v = vis[i].max(1.0);
            let c_old = old[i].max(1.0);
            let c_new = new[i].max(1.0);
            let p = if c_old <= v {
                c_old * 0.5
            } else {
                s[i] + v * 0.5
            };
            s[i] = (p * (c_new / c_old) - v * 0.5).clamp(0.0, (c_new - v).max(0.0));
        }
        scroll.0 = s;
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

/// Launch the viewer window.
///
/// `fit` selects the initial zoom mode: fit-to-window-width (the default,
/// kept live on resize until the user zooms) or the explicit `dpi`.
/// `render(page, dpi)` must return a fresh Bevy [`Image`]; returning `None`
/// keeps the current page on screen.
#[allow(clippy::too_many_arguments)]
pub fn run(
    title: String,
    probe: String,
    pages: usize,
    page: usize,
    dpi: u32,
    fit: bool,
    first: Image,
    render: impl Fn(usize, u32) -> Option<Image> + Send + Sync + 'static,
) {
    let page = page.min(pages.max(1) - 1);
    let native_w = first.width() as f32 * BASE_DPI / dpi.max(1) as f32;
    let zoom = if fit {
        fit_zoom(ASSUMED_WINDOW_W, native_w)
    } else {
        (dpi as f32 / BASE_DPI).clamp(MIN_ZOOM, MAX_ZOOM)
    };
    let dpi = render_dpi(zoom);
    let (w, h) = display_size(first.width(), first.height(), zoom, dpi);
    let (tex_w, tex_h) = (first.width(), first.height());
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.14)))
        .insert_resource(Doc {
            probe,
            pages,
            page,
            zoom,
            rendered_zoom: zoom,
            zoom_idle: RERASTER_DELAY,
            native_w,
            manual_zoom: !fit,
            tex_w,
            tex_h,
            dpi,
            rendered_page: page,
            rendered_dpi: dpi,
            image: Handle::default(),
            render: Box::new(render),
            title,
        });
    {
        let world = app.world_mut();
        let image = world
            .get_resource_mut::<Assets<Image>>()
            .expect("image assets exist")
            .add(first);
        world.get_resource_mut::<Doc>().expect("doc set").image = image;
    }
    app.add_systems(Startup, move |mut commands: Commands, doc: Res<Doc>| {
        setup_ui(&mut commands, &doc, w, h);
    })
    .add_systems(
        Update,
        (navigate, auto_fit, wheel, maybe_reraster, refresh).chain(),
    )
    .run();
}

fn setup_ui(commands: &mut Commands, doc: &Doc, w: f32, h: f32) {
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
                    overflow: Overflow::scroll(),
                    padding: UiRect::all(Val::Px(VIEWPORT_PADDING / 2.0)),
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
                    ImageNode::new(doc.image.clone()),
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

fn hud_line(doc: &Doc) -> String {
    format!(
        "{} — {} · page {}/{} · {}dpi · {:.0}% · ←/→ page · wheel scroll · Ctrl+wheel/pinch zoom · ↑/↓ zoom · 0 fit · Q quit",
        doc.title,
        doc.probe,
        doc.page + 1,
        doc.pages,
        doc.dpi,
        doc.zoom * 100.0,
    )
}

fn navigate(
    keys: Res<ButtonInput<KeyCode>>,
    mut doc: ResMut<Doc>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
    mut viewport: Query<(&ComputedNode, &mut ScrollPosition), With<Viewport>>,
) {
    if keys.just_pressed(KeyCode::ArrowRight) && doc.page + 1 < doc.pages {
        doc.page += 1;
    } else if keys.just_pressed(KeyCode::ArrowLeft) && doc.page > 0 {
        doc.page -= 1;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        let zoom = doc.zoom * KEY_ZOOM_STEP;
        apply_display_zoom(&mut doc, zoom, &mut page_nodes, &mut viewport);
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        let zoom = doc.zoom / KEY_ZOOM_STEP;
        apply_display_zoom(&mut doc, zoom, &mut page_nodes, &mut viewport);
    } else if keys.just_pressed(KeyCode::Digit0) {
        doc.manual_zoom = false;
    } else if keys.just_pressed(KeyCode::KeyQ) {
        std::process::exit(0);
    }
}

fn wheel(
    mut wheels: MessageReader<MouseWheel>,
    mut pinches: MessageReader<PinchGesture>,
    keys: Res<ButtonInput<KeyCode>>,
    mut doc: ResMut<Doc>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
    mut viewport: Query<(&ComputedNode, &mut ScrollPosition), With<Viewport>>,
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
            apply_display_zoom(&mut doc, zoom, &mut page_nodes, &mut viewport);
        }
        // Swallow the scroll too: a pinching hand also drifts, and feeding
        // that drift into the viewport fights the zoom.
        scroll_px = Vec2::ZERO;
        lines = Vec2::ZERO;
    } else if pinch != 0.0 {
        // macOS/iOS native gesture (winit never emits this on Linux).
        let zoom = doc.zoom * (pinch * WHEEL_ZOOM_SPEED).exp();
        apply_display_zoom(&mut doc, zoom, &mut page_nodes, &mut viewport);
    }
    if lines != Vec2::ZERO || scroll_px != Vec2::ZERO {
        // Wheel-up (positive y) shows earlier content: move the viewport up.
        if let Ok((_, mut pos)) = viewport.single_mut() {
            pos.0 -= scroll_px + lines * LINE_SCROLL_PX;
        }
    }
}

/// Keep fit-to-width live while the user hasn't taken over zoom.
fn auto_fit(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut doc: ResMut<Doc>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
    mut viewport: Query<(&ComputedNode, &mut ScrollPosition), With<Viewport>>,
) {
    if doc.manual_zoom {
        return;
    }
    if let Ok(window) = windows.single() {
        let zoom = fit_zoom(window.width(), doc.native_w);
        if (zoom - doc.zoom).abs() > 0.0005 {
            apply_display_zoom(&mut doc, zoom, &mut page_nodes, &mut viewport);
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
