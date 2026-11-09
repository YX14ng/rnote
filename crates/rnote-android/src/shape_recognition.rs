//! Turns freehand strokes that look like simple shapes into those shapes, like the shape
//! recognition of other note-taking apps: lines, rectangles (squares), ellipses (circles)
//! and triangles.
//!
//! A stroke is recognized when it is large enough and stays close to the outline of one of
//! the candidate shapes; the closest candidate wins. Shapes that are almost axis-aligned, or
//! almost square or round, are snapped to be exactly so.

use std::f64::consts::{FRAC_PI_2, PI};

use parry2d_f64::glamx::DAffine2;
use parry2d_f64::math::Vector2;
use parry2d_f64::shape::Cuboid;
use rnote_compose::shapes::{Ellipse, Line, Polygon, Rectangle, Shape};

/// Fewest samples a stroke needs to be considered.
const MIN_POINTS: usize = 8;
/// A line's chord must be at least this share of the stroke's length.
const LINE_STRAIGHTNESS: f64 = 0.96;
/// The ends of a closed shape may be this far apart, relative to its size.
const MAX_CLOSING_GAP: f64 = 0.3;
/// The largest mean distance from the stroke to a candidate outline, relative to the size.
const MAX_FIT_ERROR: f64 = 0.05;
/// Lines and rectangles this close to horizontal or vertical are straightened (6°).
const SNAP_ANGLE: f64 = PI / 30.0;
/// Sides or radii within this ratio of each other are made equal.
const SNAP_EQUAL: f64 = 0.1;
/// Hulls with more vertices than this skip the (cubic) triangle search.
const MAX_TRIANGLE_HULL: usize = 96;

/// The shape `points` (a freehand stroke, in document coordinates) looks like, if any.
/// Strokes whose bounding box is smaller than `min_size` along both axes are left alone, so
/// handwriting doesn't turn into shapes.
pub(crate) fn recognize(points: &[Vector2], min_size: f64) -> Option<Shape> {
    if points.len() < MIN_POINTS {
        return None;
    }
    let (mins, maxs) = bounding_box(points);
    let size = (maxs - mins).max_element();
    if size < min_size || size <= 0.0 {
        return None;
    }
    let first = points[0];
    let last = points[points.len() - 1];
    let length: f64 = points.windows(2).map(|pair| (pair[1] - pair[0]).length()).sum();

    let chord = (last - first).length();
    if chord >= LINE_STRAIGHTNESS * length {
        return Some(Shape::Line(snapped_line(first, last)));
    }
    if chord > MAX_CLOSING_GAP * size {
        return None;
    }

    let hull = convex_hull(points);
    if hull.len() < 3 {
        return None;
    }
    [
        ellipse_candidate(points, mins, maxs),
        rectangle_candidate(points, &hull),
        triangle_candidate(points, &hull),
    ]
    .into_iter()
    .flatten()
    .map(|candidate| (candidate.mean_distance(points) / size, candidate))
    .filter(|(error, _)| *error <= MAX_FIT_ERROR)
    .min_by(|(a, _), (b, _)| a.total_cmp(b))
    .map(|(_, candidate)| candidate.into_shape())
}

/// A shape being considered, in a form that's easy to measure distances to.
enum Candidate {
    Ellipse { center: Vector2, radii: Vector2 },
    Rectangle { center: Vector2, half_extents: Vector2, angle: f64 },
    Triangle([Vector2; 3]),
}

impl Candidate {
    fn mean_distance(&self, points: &[Vector2]) -> f64 {
        let total: f64 = points.iter().map(|&point| self.distance(point)).sum();
        total / points.len() as f64
    }

    fn distance(&self, point: Vector2) -> f64 {
        match self {
            Self::Ellipse { center, radii } => {
                let offset = point - *center;
                let scaled = (offset / *radii).length();
                if scaled == 0.0 {
                    radii.min_element()
                } else {
                    // Approximates the closest outline point along the ray from the center.
                    offset.length() * (1.0 - 1.0 / scaled).abs()
                }
            }
            Self::Rectangle { center, half_extents, angle } => {
                let local = Vector2::from_angle(-*angle).rotate(point - *center).abs();
                let outside = (local - *half_extents).max(Vector2::ZERO);
                if outside == Vector2::ZERO {
                    (*half_extents - local).min_element()
                } else {
                    outside.length()
                }
            }
            Self::Triangle(corners) => (0..3)
                .map(|i| segment_distance(point, corners[i], corners[(i + 1) % 3]))
                .fold(f64::INFINITY, f64::min),
        }
    }

    fn into_shape(self) -> Shape {
        match self {
            Self::Ellipse { center, radii } => {
                Shape::Ellipse(Ellipse { radii, affine: DAffine2::from_translation(center) })
            }
            Self::Rectangle { center, half_extents, angle } => Shape::Rectangle(Rectangle {
                cuboid: Cuboid::new(half_extents),
                affine: DAffine2::from_angle_translation(angle, center),
            }),
            Self::Triangle([a, b, c]) => Shape::Polygon(Polygon { start: a, path: vec![b, c] }),
        }
    }
}

/// The axis-aligned ellipse inscribed in the stroke's bounding box, made round if almost so.
fn ellipse_candidate(points: &[Vector2], mins: Vector2, maxs: Vector2) -> Option<Candidate> {
    let mut radii = (maxs - mins) / 2.0;
    if radii.min_element() <= 0.0 || points.is_empty() {
        return None;
    }
    if almost_equal(radii.x, radii.y) {
        radii = Vector2::splat((radii.x + radii.y) / 2.0);
    }
    Some(Candidate::Ellipse { center: (mins + maxs) / 2.0, radii })
}

/// The smallest rectangle around the hull, straightened and made square if almost so.
fn rectangle_candidate(points: &[Vector2], hull: &[Vector2]) -> Option<Candidate> {
    // The smallest enclosing rectangle has a side along one of the hull's edges.
    let angle = (0..hull.len())
        .map(|i| {
            let edge = hull[(i + 1) % hull.len()] - hull[i];
            edge.y.atan2(edge.x).rem_euclid(FRAC_PI_2)
        })
        .min_by(|a, b| rotated_area(hull, *a).total_cmp(&rotated_area(hull, *b)))?;
    let angle = if angle < SNAP_ANGLE || FRAC_PI_2 - angle < SNAP_ANGLE { 0.0 } else { angle };

    let (local_mins, local_maxs) = rotated_bounds(points, angle);
    let mut half_extents = (local_maxs - local_mins) / 2.0;
    if half_extents.min_element() <= 0.0 {
        return None;
    }
    if almost_equal(half_extents.x, half_extents.y) {
        half_extents = Vector2::splat((half_extents.x + half_extents.y) / 2.0);
    }
    let center = Vector2::from_angle(angle).rotate((local_mins + local_maxs) / 2.0);
    Some(Candidate::Rectangle { center, half_extents, angle })
}

/// The largest triangle with corners on the hull.
fn triangle_candidate(points: &[Vector2], hull: &[Vector2]) -> Option<Candidate> {
    if hull.len() > MAX_TRIANGLE_HULL || points.is_empty() {
        return None;
    }
    let area = |a: Vector2, b: Vector2, c: Vector2| (b - a).perp_dot(c - a).abs();
    let mut best = None;
    let mut best_area = 0.0;
    for i in 0..hull.len() {
        for j in i + 1..hull.len() {
            for k in j + 1..hull.len() {
                let candidate_area = area(hull[i], hull[j], hull[k]);
                if candidate_area > best_area {
                    best_area = candidate_area;
                    best = Some([hull[i], hull[j], hull[k]]);
                }
            }
        }
    }
    best.map(Candidate::Triangle)
}

/// A line from `start` to `end`, made horizontal or vertical if almost so.
fn snapped_line(start: Vector2, end: Vector2) -> Line {
    let delta = end - start;
    let angle = delta.y.atan2(delta.x).rem_euclid(FRAC_PI_2);
    if angle >= SNAP_ANGLE && FRAC_PI_2 - angle >= SNAP_ANGLE {
        return Line::new(start, end);
    }
    let snapped_end = if delta.x.abs() >= delta.y.abs() {
        Vector2::new(start.x + delta.x.signum() * delta.length(), start.y)
    } else {
        Vector2::new(start.x, start.y + delta.y.signum() * delta.length())
    };
    Line::new(start, snapped_end)
}

fn almost_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= SNAP_EQUAL * a.max(b)
}

fn bounding_box(points: &[Vector2]) -> (Vector2, Vector2) {
    points.iter().fold(
        (Vector2::splat(f64::INFINITY), Vector2::splat(f64::NEG_INFINITY)),
        |(mins, maxs), &point| (mins.min(point), maxs.max(point)),
    )
}

/// Bounds of `points` in a frame rotated by `angle`.
fn rotated_bounds(points: &[Vector2], angle: f64) -> (Vector2, Vector2) {
    let unrotate = Vector2::from_angle(-angle);
    let rotated: Vec<Vector2> = points.iter().map(|&point| unrotate.rotate(point)).collect();
    bounding_box(&rotated)
}

fn rotated_area(points: &[Vector2], angle: f64) -> f64 {
    let (mins, maxs) = rotated_bounds(points, angle);
    let extents = maxs - mins;
    extents.x * extents.y
}

fn segment_distance(point: Vector2, a: Vector2, b: Vector2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() == 0.0 {
        0.0
    } else {
        ((point - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    };
    (point - (a + ab * t)).length()
}

/// The convex hull, counterclockwise, by Andrew's monotone chain.
fn convex_hull(points: &[Vector2]) -> Vec<Vector2> {
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    sorted.dedup();
    if sorted.len() < 3 {
        return sorted;
    }
    let cross = |o: Vector2, a: Vector2, b: Vector2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vector2> = Vec::with_capacity(sorted.len() * 2);
    for pass in [sorted.clone(), sorted.into_iter().rev().collect()] {
        let start = hull.len();
        for point in pass {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
            {
                hull.pop();
            }
            hull.push(point);
        }
        // The last point of each pass starts the other one.
        hull.pop();
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slightly wobbly freehand version of `outline`, sampled `count` times, with a small
    /// gap between the ends like a real stroke.
    fn freehand(count: usize, outline: impl Fn(f64) -> Vector2) -> Vec<Vector2> {
        (0..count)
            .map(|i| {
                let t = i as f64 / count as f64 * 0.97;
                let wobble = Vector2::new((t * 37.0).sin(), (t * 23.0).cos()) * 1.5;
                outline(t) + wobble
            })
            .collect()
    }

    fn circle(center: Vector2, radius: f64) -> impl Fn(f64) -> Vector2 {
        move |t| center + Vector2::from_angle(t * 2.0 * PI) * radius
    }

    /// Walks the perimeter of the axis-aligned box from `mins` to `maxs`.
    fn box_outline(mins: Vector2, maxs: Vector2) -> impl Fn(f64) -> Vector2 {
        move |t| {
            let size = maxs - mins;
            let perimeter = 2.0 * (size.x + size.y);
            let mut along = t * perimeter;
            let corners = [mins, Vector2::new(maxs.x, mins.y), maxs, Vector2::new(mins.x, maxs.y)];
            for i in 0..4 {
                let (a, b) = (corners[i], corners[(i + 1) % 4]);
                let side = (b - a).length();
                if along <= side {
                    return a + (b - a) * (along / side);
                }
                along -= side;
            }
            mins
        }
    }

    #[test]
    fn recognizes_a_circle() {
        let points = freehand(120, circle(Vector2::new(300.0, 300.0), 100.0));

        let Some(Shape::Ellipse(ellipse)) = recognize(&points, 50.0) else {
            panic!("expected an ellipse");
        };
        assert!((ellipse.radii.x - ellipse.radii.y).abs() < f64::EPSILON, "{:?}", ellipse.radii);
        assert!((ellipse.radii.x - 100.0).abs() < 5.0, "{:?}", ellipse.radii);
    }

    #[test]
    fn recognizes_a_flat_ellipse() {
        let points = freehand(120, |t| {
            Vector2::new(300.0, 300.0)
                + Vector2::new((t * 2.0 * PI).cos() * 200.0, (t * 2.0 * PI).sin() * 80.0)
        });

        let Some(Shape::Ellipse(ellipse)) = recognize(&points, 50.0) else {
            panic!("expected an ellipse");
        };
        assert!(ellipse.radii.x > 2.0 * ellipse.radii.y, "{:?}", ellipse.radii);
    }

    #[test]
    fn recognizes_a_square() {
        let points =
            freehand(160, box_outline(Vector2::new(100.0, 100.0), Vector2::new(300.0, 290.0)));

        let Some(Shape::Rectangle(rectangle)) = recognize(&points, 50.0) else {
            panic!("expected a rectangle");
        };
        let half_extents = rectangle.cuboid.half_extents;
        assert!((half_extents.x - half_extents.y).abs() < f64::EPSILON, "{half_extents:?}");
        assert_eq!(rectangle.affine.matrix2, DAffine2::IDENTITY.matrix2, "should be axis-aligned");
    }

    #[test]
    fn recognizes_a_rotated_rectangle() {
        let rotation = Vector2::from_angle(PI / 6.0);
        let outline = box_outline(Vector2::new(-150.0, -60.0), Vector2::new(150.0, 60.0));
        let points = freehand(160, |t| Vector2::new(400.0, 400.0) + rotation.rotate(outline(t)));

        let Some(Shape::Rectangle(rectangle)) = recognize(&points, 50.0) else {
            panic!("expected a rectangle");
        };
        let half_extents = rectangle.cuboid.half_extents;
        assert!((half_extents.max_element() - 150.0).abs() < 6.0, "{half_extents:?}");
        assert!((half_extents.min_element() - 60.0).abs() < 6.0, "{half_extents:?}");
    }

    #[test]
    fn recognizes_a_triangle() {
        let corners =
            [Vector2::new(100.0, 400.0), Vector2::new(250.0, 100.0), Vector2::new(400.0, 400.0)];
        let outline = |t: f64| {
            let side = ((t * 3.0) as usize).min(2);
            let local = t * 3.0 - side as f64;
            corners[side] + (corners[(side + 1) % 3] - corners[side]) * local
        };
        let points = freehand(150, outline);

        assert!(
            matches!(recognize(&points, 50.0), Some(Shape::Polygon(polygon)) if polygon.path.len() == 2)
        );
    }

    #[test]
    fn recognizes_and_straightens_a_line() {
        let points: Vec<Vector2> = (0..40)
            .map(|i| {
                Vector2::new(
                    100.0 + 10.0 * f64::from(i),
                    200.0 + f64::from(i % 3) * 0.8 + f64::from(i) * 0.3,
                )
            })
            .collect();

        let Some(Shape::Line(line)) = recognize(&points, 50.0) else {
            panic!("expected a line");
        };
        assert!((line.end.y - line.start.y).abs() < f64::EPSILON, "should be horizontal: {line:?}");
    }

    #[test]
    fn leaves_small_strokes_alone() {
        let points = freehand(60, circle(Vector2::new(50.0, 50.0), 10.0));

        assert!(recognize(&points, 50.0).is_none());
    }

    #[test]
    fn leaves_open_curves_alone() {
        // A "U": closed-shape candidates don't apply, and it isn't straight.
        let points: Vec<Vector2> = (0..60)
            .map(|i| {
                let t = f64::from(i) / 59.0 * PI;
                Vector2::new(300.0 - t.cos() * 150.0, 200.0 + t.sin() * 200.0)
            })
            .collect();

        assert!(recognize(&points, 50.0).is_none());
    }

    #[test]
    fn leaves_scribbles_alone() {
        let points: Vec<Vector2> = (0..80)
            .map(|i| {
                let t = f64::from(i);
                Vector2::new(200.0 + (t * 0.9).sin() * 120.0 + t, 200.0 + (t * 1.7).cos() * 90.0)
            })
            .chain(std::iter::once(Vector2::new(210.0, 200.0)))
            .collect();

        assert!(recognize(&points, 50.0).is_none());
    }
}
