//! Tests for the shape recognizer, with synthetic freehand strokes.

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
    let points = freehand(160, box_outline(Vector2::new(100.0, 100.0), Vector2::new(300.0, 290.0)));

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
