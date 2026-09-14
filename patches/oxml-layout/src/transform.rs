//! Format-neutral two-dimensional affine transforms.

use crate::{Point, Rect};

/// A 2x3 affine transform using the PDF matrix coefficient order.
///
/// Points are transformed as `x' = a*x + c*y + e` and
/// `y' = b*x + d*y + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Transform {
    /// The affine identity transform.
    pub const IDENTITY: Transform = Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Return a rotation in degrees about `(cx, cy)`.
    pub fn rotate_about(degrees: f64, cx: f64, cy: f64) -> Transform {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Transform {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: cx - cos * cx + sin * cy,
            f: cy - sin * cx - cos * cy,
        }
    }

    /// Compose two transforms, applying `self` first and `next` second.
    pub fn then(self, next: Transform) -> Transform {
        Transform {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    /// Apply this transform to a point.
    pub fn apply(self, point: Point) -> Point {
        Point {
            x: self.a * point.x + self.c * point.y + self.e,
            y: self.b * point.x + self.d * point.y + self.f,
        }
    }

    /// Return whether all six coefficients exactly equal the identity.
    pub fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    /// Return the axis-aligned bounds of the four transformed corners.
    pub fn transform_rect_bbox(self, rect: Rect) -> Rect {
        let corners = [
            self.apply(Point {
                x: rect.x,
                y: rect.y,
            }),
            self.apply(Point {
                x: rect.x + rect.width,
                y: rect.y,
            }),
            self.apply(Point {
                x: rect.x,
                y: rect.y + rect.height,
            }),
            self.apply(Point {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
            }),
        ];

        let mut min_x = corners[0].x;
        let mut min_y = corners[0].y;
        let mut max_x = corners[0].x;
        let mut max_y = corners[0].y;
        for corner in &corners[1..] {
            min_x = min_x.min(corner.x);
            min_y = min_y.min(corner.y);
            max_x = max_x.max(corner.x);
            max_y = max_y.max(corner.y);
        }

        Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Transform;
    use crate::{Point, Rect};

    const EPSILON: f64 = 1.0e-10;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_point_close(actual: Point, expected: Point) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
    }

    #[test]
    fn identity_is_neutral_for_points_and_composition() {
        let transform = Transform {
            a: 2.0,
            b: 3.0,
            c: 5.0,
            d: 7.0,
            e: 11.0,
            f: 13.0,
        };
        let point = Point { x: 17.0, y: 19.0 };

        assert_eq!(Transform::IDENTITY.apply(point), point);
        assert_eq!(Transform::IDENTITY.then(transform), transform);
        assert_eq!(transform.then(Transform::IDENTITY), transform);
    }

    #[test]
    fn rotate_about_keeps_the_pivot_fixed() {
        let fractional = Transform::rotate_about(33.5, 10.0, 20.0);
        assert_point_close(
            fractional.apply(Point { x: 10.0, y: 20.0 }),
            Point { x: 10.0, y: 20.0 },
        );

        let quarter_turn = Transform::rotate_about(90.0, 10.0, 20.0);
        assert_point_close(
            quarter_turn.apply(Point { x: 11.0, y: 20.0 }),
            Point { x: 10.0, y: 21.0 },
        );
    }

    #[test]
    fn then_matches_the_pdf_cm_composition_order() {
        let first = Transform {
            a: 2.0,
            b: 3.0,
            c: 5.0,
            d: 7.0,
            e: 11.0,
            f: 13.0,
        };
        let next = Transform {
            a: 17.0,
            b: 19.0,
            c: 23.0,
            d: 29.0,
            e: 31.0,
            f: 37.0,
        };

        // PDF `cm` composition for first then next is next * first:
        // [103 125 246 298 517 623].
        let combined = first.then(next);
        assert_eq!(
            combined,
            Transform {
                a: 103.0,
                b: 125.0,
                c: 246.0,
                d: 298.0,
                e: 517.0,
                f: 623.0,
            }
        );
        assert_eq!(
            combined.apply(Point { x: 4.0, y: 3.0 }),
            Point {
                x: 1667.0,
                y: 2017.0
            }
        );
        assert_eq!(
            combined.apply(Point { x: 4.0, y: 3.0 }),
            next.apply(first.apply(Point { x: 4.0, y: 3.0 }))
        );
    }

    #[test]
    fn transform_rect_bbox_contains_all_four_transformed_corners() {
        let transform = Transform::rotate_about(-30.0, 0.0, 0.0);
        let rect = Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        let bounds = transform.transform_rect_bbox(rect);
        let cos = 3.0_f64.sqrt() / 2.0;

        // At -30 degrees, the top-left supplies min x, the top-right supplies
        // min y, the bottom-left supplies max y, and the bottom-right max x.
        assert_close(bounds.x, cos + 1.0);
        assert_close(bounds.y, -2.0 + 2.0 * cos);
        assert_close(bounds.width, 3.0 * cos + 2.0);
        assert_close(bounds.height, 1.5 + 4.0 * cos);

        for corner in [
            Point {
                x: rect.x,
                y: rect.y,
            },
            Point {
                x: rect.x + rect.width,
                y: rect.y,
            },
            Point {
                x: rect.x,
                y: rect.y + rect.height,
            },
            Point {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
            },
        ] {
            let point = transform.apply(corner);
            assert!(point.x >= bounds.x - EPSILON);
            assert!(point.x <= bounds.x + bounds.width + EPSILON);
            assert!(point.y >= bounds.y - EPSILON);
            assert!(point.y <= bounds.y + bounds.height + EPSILON);
        }
    }

    #[test]
    fn is_identity_is_exact() {
        assert!(Transform::IDENTITY.is_identity());
        for near_identity in [
            Transform {
                a: 1.0 + f64::EPSILON,
                ..Transform::IDENTITY
            },
            Transform {
                b: f64::EPSILON,
                ..Transform::IDENTITY
            },
            Transform {
                c: f64::EPSILON,
                ..Transform::IDENTITY
            },
            Transform {
                d: 1.0 + f64::EPSILON,
                ..Transform::IDENTITY
            },
            Transform {
                e: f64::EPSILON,
                ..Transform::IDENTITY
            },
            Transform {
                f: f64::EPSILON,
                ..Transform::IDENTITY
            },
        ] {
            assert!(!near_identity.is_identity());
        }
    }
}
