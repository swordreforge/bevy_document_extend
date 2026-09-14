//! Backend-neutral fill and stroke paint.

use crate::{Color, MediaId, Point, Rect, Transform};

/// One color stop in a gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
}

/// Fill or stroke paint independent of a rendering backend.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Solid(Color),
    Linear {
        start: Point,
        end: Point,
        stops: Vec<GradientStop>,
        extend: (bool, bool),
    },
    Radial {
        center: Point,
        radius: f64,
        focal: Point,
        stops: Vec<GradientStop>,
        extend: (bool, bool),
    },
    Tile {
        image: MediaId,
        tile: Rect,
        transform: Transform,
    },
}

impl Paint {
    /// Construct a linear gradient, degrading one stop to a solid paint.
    pub fn linear(
        start: Point,
        end: Point,
        stops: Vec<GradientStop>,
        extend: (bool, bool),
    ) -> Self {
        if let [stop] = stops.as_slice() {
            Self::Solid(stop.color)
        } else {
            Self::Linear {
                start,
                end,
                stops,
                extend,
            }
        }
    }

    /// Construct a radial gradient, degrading one stop to a solid paint.
    pub fn radial(
        center: Point,
        radius: f64,
        focal: Point,
        stops: Vec<GradientStop>,
        extend: (bool, bool),
    ) -> Self {
        if let [stop] = stops.as_slice() {
            Self::Solid(stop.color)
        } else {
            Self::Radial {
                center,
                radius,
                focal,
                stops,
                extend,
            }
        }
    }
}

/// Stroke endpoint style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Stroke segment join style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Paint and geometry used to stroke a path.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub paint: Paint,
    pub width: f64,
    pub cap: LineCap,
    pub join: LineJoin,
    pub dash: Option<Vec<f64>>,
}

impl Stroke {
    /// Construct a stroke with butt caps, miter joins, and no dash.
    pub fn new(paint: Paint, width: f64) -> Self {
        Self {
            paint,
            width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GradientStop, LineCap, LineJoin, Paint, Stroke};
    use crate::{Color, MediaId, Point, Rect, Transform};

    fn red() -> Color {
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    }

    #[test]
    fn single_stop_gradients_degrade_to_solid_at_construction() {
        let stops = vec![GradientStop {
            offset: 0.25,
            color: red(),
        }];
        assert_eq!(
            Paint::linear(
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 1.0 },
                stops.clone(),
                (false, false),
            ),
            Paint::Solid(red())
        );
        assert_eq!(
            Paint::radial(
                Point { x: 0.5, y: 0.5 },
                1.0,
                Point { x: 0.5, y: 0.5 },
                stops,
                (false, false),
            ),
            Paint::Solid(red())
        );
    }

    #[test]
    fn multiple_stop_gradients_preserve_their_geometry_and_stops() {
        let stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::BLACK,
            },
            GradientStop {
                offset: 1.0,
                color: Color::WHITE,
            },
        ];
        let paint = Paint::linear(
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
            stops.clone(),
            (true, false),
        );
        assert!(matches!(
            paint,
            Paint::Linear {
                stops: actual,
                extend: (true, false),
                ..
            } if actual == stops
        ));
    }

    #[test]
    fn stroke_new_uses_pdf_defaults() {
        let stroke = Stroke::new(Paint::Solid(red()), 2.0);
        assert_eq!(stroke.cap, LineCap::Butt);
        assert_eq!(stroke.join, LineJoin::Miter);
        assert_eq!(stroke.dash, None);
    }

    #[test]
    fn tile_paint_uses_a_media_id() {
        let media_id = MediaId::from_bytes(b"tile");
        let paint = Paint::Tile {
            image: media_id,
            tile: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            },
            transform: Transform::IDENTITY,
        };
        assert!(matches!(paint, Paint::Tile { image, .. } if image == media_id));
    }
}
