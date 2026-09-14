//! Shared Bevy UI shell for the `*_viewer` examples.
//!
//! Each format viewer resolves its CLI args (path/page/dpi), then calls
//! [`run`] with a pre-rasterized first page plus a `render` closure so the
//! UI only swaps the page [`Image`] asset on navigation — no re-parse of the
//! format is needed per key press.
//!
//! Layout: full-window scrollable page image, bottom HUD bar with the probe
//! line plus `←/→ page · wheel scroll · Ctrl+wheel / ↑/↓ zoom · 0 fit · Q quit`.
//!
//! Zoom model: continuous `zoom` multiplier (1.0 = [`BASE_DPI`] render).
//! The page Node is sized as `pixels * zoom * BASE_DPI / render_dpi`, so
//! fractional wheel steps stay smooth. Backends still take integer dpi, so
//! [`render_dpi`] rounds — a future float-dpi backend only replaces that one
//! function, the UI side is already float.
//!
//! This file is a shared module, not a runnable example: every viewer does
//! `#[path = "viewer_common/mod.rs"] mod viewer_common;` to include it.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Render resolution at zoom == 1.0.
const BASE_DPI: f32 = 150.0;
const MIN_ZOOM: f32 = 0.2;
const MAX_ZOOM: f32 = 4.0;
const KEY_ZOOM_STEP: f32 = 1.25;
const WHEEL_ZOOM_SPEED: f32 = 0.12;
const LINE_SCROLL_PX: f32 = 32.0;
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
    /// Continuous UI zoom; 1.0 shows a BASE_DPI render at native pixels.
    zoom: f32,
    /// Page width in pixels at BASE_DPI; fit-to-width target is derived from this.
    native_w: f32,
    /// False until the user zooms (or passes an explicit dpi); while false
    /// [`auto_fit`] keeps the page fitted to the window width.
    manual_zoom: bool,
    dpi: u32,
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

fn set_zoom(doc: &mut Doc, zoom: f32) {
    doc.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    doc.dpi = render_dpi(doc.zoom);
    doc.manual_zoom = true;
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
    let (w, h) = display_size(first.width(), first.height(), zoom, render_dpi(zoom));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.14)))
        .insert_resource(Doc {
            probe,
            pages,
            page,
            zoom,
            native_w,
            manual_zoom: !fit,
            dpi: render_dpi(zoom),
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
    .add_systems(Update, (navigate, auto_fit, wheel, refresh).chain())
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
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
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
        "{} — {} · page {}/{} · {}dpi · {:.0}% · ←/→ page · wheel scroll · Ctrl+wheel/↑/↓ zoom · 0 fit · Q quit",
        doc.title,
        doc.probe,
        doc.page + 1,
        doc.pages,
        doc.dpi,
        doc.zoom * 100.0,
    )
}

fn navigate(keys: Res<ButtonInput<KeyCode>>, mut doc: ResMut<Doc>) {
    if keys.just_pressed(KeyCode::ArrowRight) && doc.page + 1 < doc.pages {
        doc.page += 1;
    } else if keys.just_pressed(KeyCode::ArrowLeft) && doc.page > 0 {
        doc.page -= 1;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        let zoom = doc.zoom * KEY_ZOOM_STEP;
        set_zoom(&mut doc, zoom);
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        let zoom = doc.zoom / KEY_ZOOM_STEP;
        set_zoom(&mut doc, zoom);
    } else if keys.just_pressed(KeyCode::Digit0) {
        doc.manual_zoom = false;
    } else if keys.just_pressed(KeyCode::KeyQ) {
        std::process::exit(0);
    }
}

fn wheel(
    mut wheels: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut doc: ResMut<Doc>,
    mut scroll: Query<&mut ScrollPosition, With<Viewport>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let mut lines = Vec2::ZERO;
    let mut scroll_px = Vec2::ZERO;
    for wheel in wheels.read() {
        match wheel.unit {
            MouseScrollUnit::Line => lines += Vec2::new(wheel.x, wheel.y),
            MouseScrollUnit::Pixel => scroll_px += Vec2::new(wheel.x, wheel.y),
        }
    }
    if lines != Vec2::ZERO || scroll_px != Vec2::ZERO {
        if ctrl {
            let total = lines.y + scroll_px.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR;
            let zoom = doc.zoom * (total * WHEEL_ZOOM_SPEED).exp();
            set_zoom(&mut doc, zoom);
        } else if let Ok(mut pos) = scroll.single_mut() {
            // Wheel-up (positive y) shows earlier content: move the viewport up.
            pos.0 -= scroll_px + lines * LINE_SCROLL_PX;
        }
    }
}

/// Keep fit-to-width live while the user hasn't taken over zoom.
fn auto_fit(windows: Query<&Window, With<PrimaryWindow>>, mut doc: ResMut<Doc>) {
    if doc.manual_zoom {
        return;
    }
    if let Ok(window) = windows.single() {
        let zoom = fit_zoom(window.width(), doc.native_w);
        if (zoom - doc.zoom).abs() > 0.0005 {
            doc.zoom = zoom;
            doc.dpi = render_dpi(zoom);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    doc: Res<Doc>,
    mut images: ResMut<Assets<Image>>,
    mut page_nodes: Query<(&mut Node, &mut ImageNode), With<PageImage>>,
    mut hud: Query<&mut Text, With<HudText>>,
) {
    if !doc.is_changed() {
        return;
    }
    if let Some(image) = (doc.render)(doc.page, doc.dpi)
        && let Some(mut slot) = images.get_mut(&doc.image)
    {
        let (w, h) = display_size(image.width(), image.height(), doc.zoom, doc.dpi);
        *slot = image;
        for (mut node, _) in &mut page_nodes {
            node.width = Val::Px(w);
            node.height = Val::Px(h);
        }
    }
    for mut text in &mut hud {
        text.0 = hud_line(&doc);
    }
}
