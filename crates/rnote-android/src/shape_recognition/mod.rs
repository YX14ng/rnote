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

use self::geometry::{bounding_box, convex_hull, rotated_area, rotated_bounds, segment_distance};

mod geometry;
#[cfg(test)]
mod tests;

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
