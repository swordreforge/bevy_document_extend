//! Backend-neutral path geometry.

use crate::{Point, Rect};

/// One command in a path.
#[derive(Debug, Clone, PartialEq)]
pub enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    CurveTo { c1: Point, c2: Point, to: Point },
    Close,
}

/// The rule used to determine which regions a path fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// A backend-neutral path and its fill rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub commands: Vec<PathCommand>,
    pub fill_rule: FillRule,
}

impl Path {
    /// Return conservative bounds containing every endpoint and cubic control.
    pub fn bounds(&self) -> Option<Rect> {
        let mut points = self.commands.iter().flat_map(|command| match command {
            PathCommand::MoveTo(point) | PathCommand::LineTo(point) => [Some(*point), None, None],
            PathCommand::CurveTo { c1, c2, to } => [Some(*c1), Some(*c2), Some(*to)],
            PathCommand::Close => [None, None, None],
        });
        let first = points.find_map(|point| point)?;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);

        for point in points.flatten() {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }

        Some(Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    /// Construct a closed rectangular path.
    pub fn rect(rect: Rect) -> Path {
        let right = rect.x + rect.width;
        let bottom = rect.y + rect.height;
        Path {
            commands: vec![
                PathCommand::MoveTo(Point {
                    x: rect.x,
                    y: rect.y,
                }),
                PathCommand::LineTo(Point {
                    x: right,
                    y: rect.y,
                }),
                PathCommand::LineTo(Point {
                    x: right,
                    y: bottom,
                }),
                PathCommand::LineTo(Point {
                    x: rect.x,
                    y: bottom,
                }),
                PathCommand::Close,
            ],
            fill_rule: FillRule::NonZero,
        }
    }

    /// Construct a closed rectangle with one circular corner radius.
    pub fn round_rect(rect: Rect, radius: f64) -> Path {
        let radius = radius
            .max(0.0)
            .min(rect.width.abs() / 2.0)
            .min(rect.height.abs() / 2.0);
        if radius == 0.0 {
            return Self::rect(rect);
        }

        const KAPPA: f64 = 0.552_284_749_830_793_6;
        let right = rect.x + rect.width;
        let bottom = rect.y + rect.height;
        let control = radius * KAPPA;

        Path {
            commands: vec![
                PathCommand::MoveTo(Point {
                    x: rect.x + radius,
                    y: rect.y,
                }),
                PathCommand::LineTo(Point {
                    x: right - radius,
                    y: rect.y,
                }),
                PathCommand::CurveTo {
                    c1: Point {
                        x: right - radius + control,
                        y: rect.y,
                    },
                    c2: Point {
                        x: right,
                        y: rect.y + radius - control,
                    },
                    to: Point {
                        x: right,
                        y: rect.y + radius,
                    },
                },
                PathCommand::LineTo(Point {
                    x: right,
                    y: bottom - radius,
                }),
                PathCommand::CurveTo {
                    c1: Point {
                        x: right,
                        y: bottom - radius + control,
                    },
                    c2: Point {
                        x: right - radius + control,
                        y: bottom,
                    },
                    to: Point {
                        x: right - radius,
                        y: bottom,
                    },
                },
                PathCommand::LineTo(Point {
                    x: rect.x + radius,
                    y: bottom,
                }),
                PathCommand::CurveTo {
                    c1: Point {
                        x: rect.x + radius - control,
                        y: bottom,
                    },
                    c2: Point {
                        x: rect.x,
                        y: bottom - radius + control,
                    },
                    to: Point {
                        x: rect.x,
                        y: bottom - radius,
                    },
                },
                PathCommand::LineTo(Point {
                    x: rect.x,
                    y: rect.y + radius,
                }),
                PathCommand::CurveTo {
                    c1: Point {
                        x: rect.x,
                        y: rect.y + radius - control,
                    },
                    c2: Point {
                        x: rect.x + radius - control,
                        y: rect.y,
                    },
                    to: Point {
                        x: rect.x + radius,
                        y: rect.y,
                    },
                },
                PathCommand::Close,
            ],
            fill_rule: FillRule::NonZero,
        }
    }

    /// Construct a closed four-cubic approximation of an ellipse.
    pub fn ellipse(rect: Rect) -> Path {
        const KAPPA: f64 = 0.552_284_749_830_793_6;
        let cx = rect.x + rect.width / 2.0;
        let cy = rect.y + rect.height / 2.0;
        let rx = rect.width / 2.0;
        let ry = rect.height / 2.0;
        let dx = rx * KAPPA;
        let dy = ry * KAPPA;

        Path {
            commands: vec![
                PathCommand::MoveTo(Point { x: cx + rx, y: cy }),
                PathCommand::CurveTo {
                    c1: Point {
                        x: cx + rx,
                        y: cy + dy,
                    },
                    c2: Point {
                        x: cx + dx,
                        y: cy + ry,
                    },
                    to: Point { x: cx, y: cy + ry },
                },
                PathCommand::CurveTo {
                    c1: Point {
                        x: cx - dx,
                        y: cy + ry,
                    },
                    c2: Point {
                        x: cx - rx,
                        y: cy + dy,
                    },
                    to: Point { x: cx - rx, y: cy },
                },
                PathCommand::CurveTo {
                    c1: Point {
                        x: cx - rx,
                        y: cy - dy,
                    },
                    c2: Point {
                        x: cx - dx,
                        y: cy - ry,
                    },
                    to: Point { x: cx, y: cy - ry },
                },
                PathCommand::CurveTo {
                    c1: Point {
                        x: cx + dx,
                        y: cy - ry,
                    },
                    c2: Point {
                        x: cx + rx,
                        y: cy - dy,
                    },
                    to: Point { x: cx + rx, y: cy },
                },
                PathCommand::Close,
            ],
            fill_rule: FillRule::NonZero,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FillRule, Path, PathCommand};
    use crate::{Point, Rect};

    #[test]
    fn empty_path_has_no_bounds() {
        let path = Path {
            commands: Vec::new(),
            fill_rule: FillRule::NonZero,
        };
        assert_eq!(path.bounds(), None);
    }

    #[test]
    fn bounds_include_cubic_control_points() {
        let path = Path {
            commands: vec![
                PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }),
                PathCommand::CurveTo {
                    c1: Point { x: -2.0, y: 4.0 },
                    c2: Point { x: 6.0, y: -3.0 },
                    to: Point { x: 1.0, y: 2.0 },
                },
            ],
            fill_rule: FillRule::NonZero,
        };
        assert_eq!(
            path.bounds(),
            Some(Rect {
                x: -2.0,
                y: -3.0,
                width: 8.0,
                height: 7.0,
            })
        );
    }

    #[test]
    fn rect_constructor_emits_a_closed_nonzero_path() {
        let path = Path::rect(Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        });
        assert_eq!(path.fill_rule, FillRule::NonZero);
        assert!(matches!(path.commands.last(), Some(PathCommand::Close)));
    }

    #[test]
    fn round_rect_clamps_the_radius_to_half_the_shorter_side() {
        let rect = Rect {
            x: 1.0,
            y: 2.0,
            width: 10.0,
            height: 4.0,
        };
        assert_eq!(Path::round_rect(rect, 99.0), Path::round_rect(rect, 2.0));
        assert_eq!(Path::round_rect(rect, -1.0), Path::rect(rect));
    }

    #[test]
    fn round_rect_with_zero_radius_matches_rect() {
        let rect = Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        assert_eq!(Path::round_rect(rect, 0.0), Path::rect(rect));
    }

    #[test]
    fn ellipse_path_bounds_contain_the_ellipse_and_lie_within_its_control_hull() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        assert_eq!(Path::ellipse(rect).bounds(), Some(rect));
    }

    #[test]
    fn bounds_do_not_depend_on_the_fill_rule() {
        let commands = Path::rect(Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        })
        .commands;
        let nonzero = Path {
            commands: commands.clone(),
            fill_rule: FillRule::NonZero,
        };
        let evenodd = Path {
            commands,
            fill_rule: FillRule::EvenOdd,
        };
        assert_eq!(nonzero.bounds(), evenodd.bounds());
    }
}
