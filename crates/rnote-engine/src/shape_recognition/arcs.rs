//! Brackets: open strokes that bend once to one side, like a tall "(" or ")" around a
//! matrix, made into a symmetric arc.

use std::f64::consts::FRAC_PI_2;

use parry2d_f64::math::Vector2;
use rnote_compose::shapes::QuadraticBezier;

use super::{MAX_FIT_ERROR, SNAP_ANGLE, polygons};

/// The least a bracket bends: its bulge over the length between its ends. Less is a line.
const MIN_BULGE: f64 = 0.06;
/// The most a bracket bends. More is a "U" or a "C", left as drawn.
const MAX_BULGE: f64 = 0.35;
/// How far, relative to the length between the ends, the hand may stray to the other side.
const OTHER_SIDE_TOLERANCE: f64 = 0.02;
/// Samples of the arc that distances to it are measured against.
const ARC_SAMPLES: usize = 64;

/// The symmetric arc `points` (an open stroke of size `size`) looks like, if it bends once to
/// one side without corners, and neither too little nor too much.
pub(super) fn bracket(points: &[Vector2], size: f64) -> Option<QuadraticBezier> {
    let first = *points.first()?;
    let last = *points.last()?;
    let chord = last - first;
    let length = chord.length();
    if length <= 0.0 {
        return None;
    }
    let normal = Vector2::new(-chord.y, chord.x) / length;
    let offsets = points.iter().map(|&point| (point - first).dot(normal));
    let (low, high) = offsets.fold((0.0_f64, 0.0_f64), |(low, high), offset| {
        (low.min(offset), high.max(offset))
    });
    // Which side it bulges to, and how far the hand strayed to the other one.
    let (bulge, stray, side) = if high >= -low {
        (high, -low, 1.0)
    } else {
        (-low, high, -1.0)
    };
    let bends = (MIN_BULGE..=MAX_BULGE).contains(&(bulge / length));
    if !bends || stray > OTHER_SIDE_TOLERANCE * length {
        return None;
    }
    // A "<" bulges like a bracket too, but turns at a corner.
    if polygons::open_vertices(points, size).is_some() {
        return None;
    }
    let (start, end) = straightened(first, last);
    let ends = end - start;
    let normal = Vector2::new(-ends.y, ends.x) / ends.length() * side;
    // A quadratic Bézier bends half as far as its control point.
    let curve = QuadraticBezier {
        start,
        cp: (start + end) / 2.0 + normal * 2.0 * bulge,
        end,
    };
    (mean_distance(points, &curve) / size <= MAX_FIT_ERROR).then_some(curve)
}

/// The ends, turned around their middle to stand exactly upright or level when they almost
/// do.
fn straightened(first: Vector2, last: Vector2) -> (Vector2, Vector2) {
    let delta = last - first;
    let angle = delta.y.atan2(delta.x).rem_euclid(FRAC_PI_2);
    if angle >= SNAP_ANGLE && FRAC_PI_2 - angle >= SNAP_ANGLE {
        return (first, last);
    }
    let middle = (first + last) / 2.0;
    let half = delta.length() / 2.0;
    let along = if delta.x.abs() >= delta.y.abs() {
        Vector2::new(delta.x.signum() * half, 0.0)
    } else {
        Vector2::new(0.0, delta.y.signum() * half)
    };
    (middle - along, middle + along)
}

/// The mean distance from `points` to `curve`.
fn mean_distance(points: &[Vector2], curve: &QuadraticBezier) -> f64 {
    let samples: Vec<Vector2> = (0..=ARC_SAMPLES)
        .map(|i| {
            let t = i as f64 / ARC_SAMPLES as f64;
            curve.start * (1.0 - t).powi(2)
                + curve.cp * (2.0 * t * (1.0 - t))
                + curve.end * t.powi(2)
        })
        .collect();
    let total: f64 = points
        .iter()
        .map(|&point| {
            samples
                .windows(2)
                .map(|pair| super::geometry::segment_distance(point, pair[0], pair[1]))
                .fold(f64::INFINITY, f64::min)
        })
        .sum();
    total / points.len() as f64
}
