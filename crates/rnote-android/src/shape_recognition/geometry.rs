//! Plain geometry helpers for the recognizer.

use parry2d_f64::math::Vector2;

pub(super) fn bounding_box(points: &[Vector2]) -> (Vector2, Vector2) {
    points.iter().fold(
        (Vector2::splat(f64::INFINITY), Vector2::splat(f64::NEG_INFINITY)),
        |(mins, maxs), &point| (mins.min(point), maxs.max(point)),
    )
}

/// Bounds of `points` in a frame rotated by `angle`.
pub(super) fn rotated_bounds(points: &[Vector2], angle: f64) -> (Vector2, Vector2) {
    let unrotate = Vector2::from_angle(-angle);
    let rotated: Vec<Vector2> = points.iter().map(|&point| unrotate.rotate(point)).collect();
    bounding_box(&rotated)
}

pub(super) fn rotated_area(points: &[Vector2], angle: f64) -> f64 {
    let (mins, maxs) = rotated_bounds(points, angle);
    let extents = maxs - mins;
    extents.x * extents.y
}

pub(super) fn segment_distance(point: Vector2, a: Vector2, b: Vector2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() == 0.0 {
        0.0
    } else {
        ((point - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    };
    (point - (a + ab * t)).length()
}

/// The convex hull, counterclockwise, by Andrew's monotone chain.
pub(super) fn convex_hull(points: &[Vector2]) -> Vec<Vector2> {
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
