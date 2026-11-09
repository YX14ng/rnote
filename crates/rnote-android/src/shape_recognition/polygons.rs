//! Polygons, polylines and arrows: shapes made of straight sides meeting at corners.
//!
//! Corners are found by simplifying the stroke (Ramer–Douglas–Peucker), then keeping only
//! vertices where the stroke turns sharply: the turn has to happen within a short stretch of
//! the stroke around the vertex. That tells a hexagon from a circle, which turns as much
//! overall but evenly.

use std::f64::consts::{PI, TAU};

use parry2d_f64::math::Vector2;

use super::geometry::segment_distance;

/// Simplification tolerance, relative to the stroke's size.
const SIMPLIFY_TOLERANCE: f64 = 0.03;
/// Vertices where the outline turns less than this are not corners (25°).
const MIN_CORNER_TURN: f64 = 25.0 * PI / 180.0;
/// Stretch of the stroke on each side of a vertex where its turn must happen, relative to
/// the stroke's length.
const SHARPNESS_WINDOW: f64 = 0.02;
/// Share of a corner's turn that must happen within that stretch, on average.
const MIN_SHARPNESS: f64 = 0.55;
/// The most corners a recognized polygon may have.
const MAX_POLYGON_CORNERS: usize = 12;
/// The most vertices, ends included, a recognized polyline may have.
const MAX_POLYLINE_VERTICES: usize = 8;
/// Regular polygons may have sides this far apart from their mean, relatively.
const REGULAR_SIDE_TOLERANCE: f64 = 0.18;
/// ... and turns this far from the regular one (12°).
const REGULAR_TURN_TOLERANCE: f64 = 12.0 * PI / 180.0;
/// Corners within this of 90° count as right angles (20°).
const MAX_RIGHT_ANGLE_ERROR: f64 = 20.0 * PI / 180.0;
/// Regular shapes this close to standing on a side or a corner are rotated to it (8°).
const SNAP_ROTATION: f64 = 8.0 * PI / 180.0;

/// The corners of a closed stroke, if it is made of straight sides.
pub(super) fn closed_corners(points: &[Vector2], size: f64) -> Option<Vec<Vector2>> {
    let path = Path::new(points, /* closed */ true);
    let far = (1..points.len()).max_by(|&a, &b| {
        (points[a] - points[0]).length().total_cmp(&(points[b] - points[0]).length())
    })?;
    let tolerance = SIMPLIFY_TOLERANCE * size;
    let mut indices = simplify(points, 0, far, tolerance);
    indices.pop();
    let mut closing: Vec<Vector2> = points[far..].to_vec();
    closing.push(points[0]);
    let back = simplify(&closing, 0, closing.len() - 1, tolerance);
    indices.extend(back[..back.len() - 1].iter().map(|&i| i + far));
    let corners = drop_shallow_vertices(&path, indices, /* closed */ true);
    (3..=MAX_POLYGON_CORNERS).contains(&corners.len()).then_some(())?;
    path.is_sharp(&corners).then(|| corners.iter().map(|&i| points[i]).collect())
}

/// The vertices of an open stroke, ends included, if it is made of straight segments with
/// at least one corner.
pub(super) fn open_vertices(points: &[Vector2], size: f64) -> Option<Vec<Vector2>> {
    let path = Path::new(points, /* closed */ false);
    let indices = simplify(points, 0, points.len() - 1, SIMPLIFY_TOLERANCE * size);
    let vertices = drop_shallow_vertices(&path, indices, /* closed */ false);
    (3..=MAX_POLYLINE_VERTICES).contains(&vertices.len()).then_some(())?;
    path.is_sharp(&vertices[1..vertices.len() - 1])
        .then(|| vertices.iter().map(|&i| points[i]).collect())
}

/// Whether every corner of the outline through `corners` is close to a right angle.
pub(super) fn right_angled(corners: &[Vector2]) -> bool {
    let n = corners.len();
    (0..n).all(|i| {
        let angle = turn(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n]).abs();
        (angle - PI / 2.0).abs() <= MAX_RIGHT_ANGLE_ERROR
    })
}

/// `corners` as a regular polygon or star if they almost are one, else unchanged.
pub(super) fn regularized(corners: Vec<Vector2>) -> Vec<Vector2> {
    let n = corners.len();
    let center = corners.iter().copied().sum::<Vector2>() / n as f64;
    let turns: Vec<f64> =
        (0..n).map(|i| turn(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n])).collect();
    let convex = turns.iter().all(|t| *t > 0.0) || turns.iter().all(|t| *t < 0.0);
    if convex && n != 4 {
        let sides: Vec<f64> =
            (0..n).map(|i| (corners[(i + 1) % n] - corners[i]).length()).collect();
        let mean_side = sides.iter().sum::<f64>() / n as f64;
        let regular_turn = TAU / n as f64;
        let even_sides =
            sides.iter().all(|side| (side - mean_side).abs() <= REGULAR_SIDE_TOLERANCE * mean_side);
        let even_turns =
            turns.iter().all(|t| (t.abs() - regular_turn).abs() <= REGULAR_TURN_TOLERANCE);
        if even_sides && even_turns {
            let radius = corners.iter().map(|c| (*c - center).length()).sum::<f64>() / n as f64;
            return regular_points(center, &[radius], n, phase_of(&corners, center, n));
        }
    }
    if n == 10
        && let Some(star) = regular_star(&corners, center)
    {
        return star;
    }
    corners
}

/// A regular star if the 10 `corners` alternate between tips and inner corners evenly.
fn regular_star(corners: &[Vector2], center: Vector2) -> Option<Vec<Vector2>> {
    let radii: Vec<f64> = corners.iter().map(|c| (*c - center).length()).collect();
    // Tips are the corners at even or at odd positions, whichever lie farther out.
    let first_tip = usize::from(radii[1] > radii[0]);
    let tips: Vec<f64> = radii.iter().skip(first_tip).step_by(2).copied().collect();
    let inner: Vec<f64> = radii.iter().skip(1 - first_tip).step_by(2).copied().collect();
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let (tip_radius, inner_radius) = (mean(&tips), mean(&inner));
    let even = |values: &[f64], mean: f64| {
        values.iter().all(|v| (v - mean).abs() <= REGULAR_SIDE_TOLERANCE * mean)
    };
    let spaced = (0..10).all(|i| {
        let a = corners[i] - center;
        let b = corners[(i + 1) % 10] - center;
        (a.angle_to(b).abs() - TAU / 10.0).abs() <= REGULAR_TURN_TOLERANCE
    });
    if inner_radius >= 0.85 * tip_radius
        || !even(&tips, tip_radius)
        || !even(&inner, inner_radius)
        || !spaced
    {
        return None;
    }
    let tip_corners: Vec<Vector2> = corners.iter().skip(first_tip).step_by(2).copied().collect();
    let phase = phase_of(&tip_corners, center, 5);
    Some(regular_points(center, &[tip_radius, inner_radius], 10, phase))
}

/// `count` points around `center` starting at angle `phase`, with radii cycling through
/// `radii` (one for a polygon, tip and inner for a star).
fn regular_points(center: Vector2, radii: &[f64], count: usize, phase: f64) -> Vec<Vector2> {
    (0..count)
        .map(|i| {
            let angle = phase + TAU * i as f64 / count as f64;
            center + Vector2::from_angle(angle) * radii[i % radii.len()]
        })
        .collect()
}

/// The angle of the first of `corners` around `center`, turned to stand on a corner or a
/// side (pointing up, or with a flat bottom) when almost so. `symmetry` corners repeat it.
fn phase_of(corners: &[Vector2], center: Vector2, symmetry: usize) -> f64 {
    let step = TAU / symmetry as f64;
    let offset = corners[0] - center;
    let phase = offset.y.atan2(offset.x);
    // Up is -90° on screen, where y grows downwards; half a step more puts a side down.
    for upright in [-PI / 2.0, -PI / 2.0 + step / 2.0] {
        let delta = (phase - upright).rem_euclid(step);
        let delta = if delta > step / 2.0 { delta - step } else { delta };
        if delta.abs() <= SNAP_ROTATION {
            return phase - delta;
        }
    }
    phase
}

/// The arrow `vertices` (of an open stroke) were drawn as, if any: a shaft, then the head
/// as two barbs drawn out from the tip and back. Returns `(start, tip)`.
pub(super) fn arrow(vertices: &[Vector2]) -> Option<(Vector2, Vector2)> {
    let shaft_then_head = |v: &[Vector2]| -> Option<(Vector2, Vector2)> {
        let [tail, tip, barb_a, tip_again, barb_b] = <[Vector2; 5]>::try_from(v).ok()?;
        let shaft = tip - tail;
        let length = shaft.length();
        let barb_ok = |barb: Vector2| {
            let barb_length = (barb - tip).length();
            // Barbs point back along the shaft and are shorter than it.
            (0.08..=0.6).contains(&(barb_length / length)) && (barb - tip).dot(-shaft) > 0.0
        };
        let sides_differ =
            shaft.perp_dot(barb_a - tip).signum() != shaft.perp_dot(barb_b - tip).signum();
        ((tip_again - tip).length() <= 0.25 * length
            && barb_ok(barb_a)
            && barb_ok(barb_b)
            && sides_differ)
            .then_some((tail, tip))
    };
    shaft_then_head(vertices).or_else(|| {
        let reversed: Vec<Vector2> = vertices.iter().rev().copied().collect();
        shaft_then_head(&reversed)
    })
}

/// Mean distance from `points` to the polyline through `vertices`, closed or not.
pub(super) fn mean_distance_to(points: &[Vector2], vertices: &[Vector2], closed: bool) -> f64 {
    let n = vertices.len();
    let segments = if closed { n } else { n - 1 };
    let total: f64 = points
        .iter()
        .map(|&point| {
            (0..segments)
                .map(|i| segment_distance(point, vertices[i], vertices[(i + 1) % n]))
                .fold(f64::INFINITY, f64::min)
        })
        .sum();
    total / points.len() as f64
}

/// The signed angle the outline turns by at `vertex`.
fn turn(previous: Vector2, vertex: Vector2, next: Vector2) -> f64 {
    (vertex - previous).angle_to(next - vertex)
}

/// Indices of the points kept by Ramer–Douglas–Peucker between `first` and `last`, both
/// included, in order.
fn simplify(points: &[Vector2], first: usize, last: usize, tolerance: f64) -> Vec<usize> {
    let mut kept = vec![first];
    let mut stack = vec![(first, last)];
    let mut result = Vec::new();
    while let Some((a, b)) = stack.pop() {
        let farthest = (a + 1..b)
            .map(|i| (i, segment_distance(points[i], points[a], points[b])))
            .max_by(|x, y| x.1.total_cmp(&y.1));
        match farthest {
            Some((i, distance)) if distance > tolerance => {
                stack.push((i, b));
                stack.push((a, i));
            }
            _ => result.push(b),
        }
    }
    kept.extend(result);
    kept
}

/// Removes, one at a time, the vertex where the outline turns least, while that turn is
/// below [`MIN_CORNER_TURN`]. The ends of an open stroke stay.
fn drop_shallow_vertices(path: &Path, mut indices: Vec<usize>, closed: bool) -> Vec<usize> {
    loop {
        let n = indices.len();
        let candidates = if closed { 0..n } else { 1..n.saturating_sub(1) };
        if n < 3 {
            return indices;
        }
        let shallowest = candidates
            .map(|i| {
                let previous = path.points[indices[(i + n - 1) % n]];
                let next = path.points[indices[(i + 1) % n]];
                (i, turn(previous, path.points[indices[i]], next).abs())
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match shallowest {
            Some((i, angle)) if angle < MIN_CORNER_TURN => {
                indices.remove(i);
            }
            _ => return indices,
        }
    }
}

/// A stroke with its length measured along the way, to look at stretches of it.
struct Path<'a> {
    points: &'a [Vector2],
    /// Distance along the stroke at each point.
    along: Vec<f64>,
    length: f64,
    closed: bool,
}

impl<'a> Path<'a> {
    fn new(points: &'a [Vector2], closed: bool) -> Self {
        let mut along = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for (i, point) in points.iter().enumerate() {
            if i > 0 {
                total += (*point - points[i - 1]).length();
            }
            along.push(total);
        }
        let length =
            if closed { total + (points[0] - points[points.len() - 1]).length() } else { total };
        Self { points, along, length, closed }
    }

    /// The point at distance `s` along the stroke, wrapping around closed ones.
    fn point_at(&self, s: f64) -> Vector2 {
        let s = if self.closed { s.rem_euclid(self.length) } else { s.clamp(0.0, self.length) };
        let i = self.along.partition_point(|&a| a <= s);
        if i == 0 {
            return self.points[0];
        }
        let (a, start) = (self.points[i - 1], self.along[i - 1]);
        let (b, end) = if i < self.points.len() {
            (self.points[i], self.along[i])
        } else {
            (self.points[0], self.length)
        };
        if end <= start { a } else { a + (b - a) * ((s - start) / (end - start)) }
    }

    /// Whether the stroke turns sharply at the points `vertices` (indices): mostly within a
    /// short stretch around each, rather than spread along the way like a curve.
    fn is_sharp(&self, vertices: &[usize]) -> bool {
        if vertices.is_empty() {
            return true;
        }
        let window = SHARPNESS_WINDOW * self.length;
        let total: f64 = vertices
            .iter()
            .map(|&i| {
                let s = self.along[i];
                let before = self.point_at(s - window);
                let after = self.point_at(s + window);
                let local = turn(before, self.points[i], after).abs();
                // Compare with the turn between points much farther away along the stroke.
                let wide = 4.0 * window;
                let outline =
                    turn(self.point_at(s - wide), self.points[i], self.point_at(s + wide)).abs();
                if outline <= f64::EPSILON { 0.0 } else { (local / outline).min(1.0) }
            })
            .sum();
        total / vertices.len() as f64 >= MIN_SHARPNESS
    }
}
