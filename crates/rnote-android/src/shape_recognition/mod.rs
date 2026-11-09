//! Turns freehand strokes that look like shapes into those shapes, like the shape recognition
//! of other note-taking apps: lines, rectangles (squares), ellipses (circles), triangles,
//! any other polygon with sharp corners (pentagons, hexagons, stars, arrows drawn as an
//! outline, ...), and open strokes made of straight segments (polylines, arrows).
//!
//! A stroke is recognized when it is large enough and stays close to the outline of one of
//! the candidate shapes; the closest candidate wins. Shapes that are almost axis-aligned, or
//! almost square, round or regular, are snapped to be exactly so.

use std::f64::consts::{FRAC_PI_2, PI};

use parry2d_f64::glamx::DAffine2;
use parry2d_f64::math::Vector2;
use parry2d_f64::shape::Cuboid;
use rnote_compose::shapes::{Arrow, Ellipse, Line, Polygon, Polyline, Rectangle, Shape};

use self::geometry::{bounding_box, convex_hull, rotated_area, rotated_bounds};

mod geometry;
mod polygons;
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
/// How much more a general polygon's fit error counts: when a rectangle or triangle fits
/// almost as well, it wins, since those come out straightened.
const POLYGON_ERROR_WEIGHT: f64 = 1.5;

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
        return open_shape(points, size);
    }

    let hull = convex_hull(points);
    if hull.len() < 3 {
        return None;
    }
    let fits = |candidate: Candidate| {
        let error = candidate.mean_distance(points) / size;
        (error <= MAX_FIT_ERROR).then_some((error, candidate))
    };
    let corners = polygons::closed_corners(points, size);
    // Four corners that aren't right angles make a rhombus, a trapezoid, ..., however well
    // a rectangle would fit.
    let rectangle = match &corners {
        Some(corners) if corners.len() == 4 && !polygons::right_angled(corners) => None,
        _ => rectangle_candidate(points, &hull),
    };
    let mut fitting: Vec<(f64, Candidate)> =
        [ellipse_candidate(points, mins, maxs), rectangle, triangle_candidate(points, &hull)]
            .into_iter()
            .flatten()
            .filter_map(fits)
            .collect();
    // A general polygon only competes where no straightened rectangle or triangle fits.
    let has_rectangle = fitting.iter().any(|(_, c)| matches!(c, Candidate::Rectangle { .. }));
    let has_triangle = fitting.iter().any(|(_, c)| matches!(c, Candidate::Polygon { .. }));
    if let Some(corners) = corners {
        let covered = (corners.len() == 4 && has_rectangle) || (corners.len() == 3 && has_triangle);
        if !covered {
            let polygon = Candidate::Polygon {
                corners: polygons::regularized(corners),
                weight: POLYGON_ERROR_WEIGHT,
            };
            fitting.extend(fits(polygon));
        }
    }
    fitting
        .into_iter()
        .min_by(|(a, x), (b, y)| (a * x.weight()).total_cmp(&(b * y.weight())))
        .map(|(_, candidate)| candidate.into_shape())
}

/// An open stroke made of straight segments: an arrow if drawn as one, else a polyline.
fn open_shape(points: &[Vector2], size: f64) -> Option<Shape> {
    let vertices = polygons::open_vertices(points, size)?;
    if let Some((start, tip)) = polygons::arrow(&vertices) {
        return Some(Shape::Arrow(Arrow::new(start, tip)));
    }
    let error = polygons::mean_distance_to(points, &vertices, /* closed */ false) / size;
    (error <= MAX_FIT_ERROR)
        .then(|| Shape::Polyline(Polyline { start: vertices[0], path: vertices[1..].to_vec() }))
}

/// A shape being considered, in a form that's easy to measure distances to.
enum Candidate {
    Ellipse {
        center: Vector2,
        radii: Vector2,
    },
    Rectangle {
        center: Vector2,
        half_extents: Vector2,
        angle: f64,
    },
    /// A closed outline through `corners`; `weight` scales its fit error when comparing.
    Polygon {
        corners: Vec<Vector2>,
        weight: f64,
    },
}

impl Candidate {
    fn weight(&self) -> f64 {
        match self {
            Self::Polygon { weight, .. } => *weight,
            Self::Ellipse { .. } | Self::Rectangle { .. } => 1.0,
        }
    }

    fn mean_distance(&self, points: &[Vector2]) -> f64 {
        if let Self::Polygon { corners, .. } = self {
            return polygons::mean_distance_to(points, corners, /* closed */ true);
        }
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
            Self::Polygon { corners, .. } => polygons::mean_distance_to(&[point], corners, true),
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
            Self::Polygon { corners, .. } => {
                Shape::Polygon(Polygon { start: corners[0], path: corners[1..].to_vec() })
            }
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
    best.map(|corners| Candidate::Polygon { corners: corners.to_vec(), weight: 1.0 })
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
