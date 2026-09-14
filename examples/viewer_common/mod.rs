//! Shared Bevy UI shell for the `*_viewer` examples.
//!
//! Each format viewer resolves its CLI args (path/page/dpi), then calls
//! [`run`] with a pre-rasterized first page plus a `render` closure so the
//! UI only swaps the page [`Image`] asset on navigation — no re-parse of the
//! format is needed per key press.
//!
//! Layout: full-window scrollable page image, bottom HUD bar with the probe
//! line plus `←/→ page · ↑/↓ or wheel zoom · Q quit`.
//!
//! This file is a shared module, not a runnable example: every viewer does
//! `#[path = "viewer_common/mod.rs"] mod viewer_common;` to include it.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

const ZOOM_STEPS: [u32; 5] = [72, 100, 150, 200, 300];

#[derive(Resource)]
struct Doc {
    title: String,
    probe: String,
    pages: usize,
    page: usize,
    zoom: usize,
    dpi: u32,
    image: Handle<Image>,
    render: Box<dyn Fn(usize, u32) -> Option<Image> + Send + Sync>,
}

#[derive(Component)]
struct PageImage;

#[derive(Component)]
struct HudText;

fn default_zoom(dpi: u32) -> usize {
    ZOOM_STEPS
        .iter()
        .position(|&step| step == dpi)
        .unwrap_or(ZOOM_STEPS.iter().position(|&step| step == 150).unwrap_or(0))
}

/// Launch the viewer window.
///
/// `render(page, dpi)` must return a fresh Bevy [`Image`] for the requested
/// page; returning `None` keeps the current page on screen.
pub fn run(
    title: String,
    probe: String,
    pages: usize,
    page: usize,
    dpi: u32,
    first: Image,
    render: impl Fn(usize, u32) -> Option<Image> + Send + Sync + 'static,
) {
    let page = page.min(pages.max(1) - 1);
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.14)))
        .insert_resource(Doc {
            probe,
            pages,
            page,
            zoom: default_zoom(dpi),
            dpi: ZOOM_STEPS[default_zoom(dpi)],
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
    app.add_systems(Startup, setup_ui)
        .add_systems(Update, (navigate, zoom_wheel, refresh))
        .run();
}

fn setup_ui(mut commands: Commands, doc: Res<Doc>, images: Res<Assets<Image>>) {
    commands.spawn(Camera2d);
    let (w, h) = images
        .get(&doc.image)
        .map(|img| (img.width() as f32, img.height() as f32))
        .unwrap_or((800.0, 600.0));
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
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::scroll(),
                    padding: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.16, 0.18)),
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
                    Text::new(hud_line(&doc)),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::WHITE),
                ));
            });
        });
}

fn hud_line(doc: &Doc) -> String {
    format!(
        "{} — {} · page {}/{} · {}dpi · ←/→ page · ↑/↓ or wheel zoom · Q quit",
        doc.title,
        doc.probe,
        doc.page + 1,
        doc.pages,
        doc.dpi,
    )
}

fn navigate(keys: Res<ButtonInput<KeyCode>>, mut doc: ResMut<Doc>) {
    if keys.just_pressed(KeyCode::ArrowRight) && doc.page + 1 < doc.pages {
        doc.page += 1;
    } else if keys.just_pressed(KeyCode::ArrowLeft) && doc.page > 0 {
        doc.page -= 1;
    } else if keys.just_pressed(KeyCode::ArrowUp) && doc.zoom + 1 < ZOOM_STEPS.len() {
        doc.zoom += 1;
        doc.dpi = ZOOM_STEPS[doc.zoom];
    } else if keys.just_pressed(KeyCode::ArrowDown) && doc.zoom > 0 {
        doc.zoom -= 1;
        doc.dpi = ZOOM_STEPS[doc.zoom];
    } else if keys.just_pressed(KeyCode::KeyQ) {
        std::process::exit(0);
    }
}

fn zoom_wheel(mut wheels: MessageReader<MouseWheel>, mut doc: ResMut<Doc>) {
    let mut delta = 0.0;
    for wheel in wheels.read() {
        delta += wheel.y;
    }
    if delta > 0.0 && doc.zoom + 1 < ZOOM_STEPS.len() {
        doc.zoom += 1;
        doc.dpi = ZOOM_STEPS[doc.zoom];
    } else if delta < 0.0 && doc.zoom > 0 {
        doc.zoom -= 1;
        doc.dpi = ZOOM_STEPS[doc.zoom];
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
        let (w, h) = (image.width(), image.height());
        *slot = image;
        for (mut node, _) in &mut page_nodes {
            node.width = Val::Px(w as f32);
            node.height = Val::Px(h as f32);
        }
    }
    for mut text in &mut hud {
        text.0 = hud_line(&doc);
    }
}
