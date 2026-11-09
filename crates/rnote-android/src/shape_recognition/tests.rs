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

/// Walks the closed outline through `corners`, proportionally to the length of each side.
fn polygon_outline(corners: Vec<Vector2>) -> impl Fn(f64) -> Vector2 {
    move |t| walk(&corners, true, t)
}

fn walk(vertices: &[Vector2], closed: bool, t: f64) -> Vector2 {
    let n = vertices.len();
    let segments: Vec<(Vector2, Vector2)> =
        (0..if closed { n } else { n - 1 }).map(|i| (vertices[i], vertices[(i + 1) % n])).collect();
    let total: f64 = segments.iter().map(|(a, b)| (*b - *a).length()).sum();
    let mut along = t.clamp(0.0, 1.0) * total;
    for (a, b) in &segments {
        let side = (*b - *a).length();
        if along <= side {
            return *a + (*b - *a) * (along / side);
        }
        along -= side;
    }
    segments.last().map_or(vertices[0], |(_, b)| *b)
}

/// The corners of a regular `n`-gon around (400, 400), slightly uneven like a drawn one.
fn drawn_regular_polygon(n: usize, radius: f64) -> Vec<Vector2> {
    (0..n)
        .map(|i| {
            let angle = -PI / 2.0 + 2.0 * PI * i as f64 / n as f64 + 0.03 * (i as f64 * 1.7).sin();
            Vector2::new(400.0, 400.0)
                + Vector2::from_angle(angle) * radius * (1.0 + 0.03 * (i as f64 * 2.3).cos())
        })
        .collect()
}

fn polygon_corners(shape: Option<Shape>) -> Vec<Vector2> {
    match shape {
        Some(Shape::Polygon(polygon)) => {
            std::iter::once(polygon.start).chain(polygon.path).collect()
        }
        other => panic!("expected a polygon, got {other:?}"),
    }
}

fn side_lengths(corners: &[Vector2]) -> Vec<f64> {
    (0..corners.len()).map(|i| (corners[(i + 1) % corners.len()] - corners[i]).length()).collect()
}

fn assert_regular(corners: &[Vector2]) {
    let sides = side_lengths(corners);
    let mean = sides.iter().sum::<f64>() / sides.len() as f64;
    assert!(sides.iter().all(|side| (side - mean).abs() < 1e-6), "uneven sides: {sides:?}");
}

#[test]
fn recognizes_a_pentagon_and_makes_it_regular() {
    let points = freehand(200, polygon_outline(drawn_regular_polygon(5, 150.0)));

    let corners = polygon_corners(recognize(&points, 50.0));

    assert_eq!(corners.len(), 5);
    assert_regular(&corners);
    // Pointing up, like it was drawn.
    let top = corners.iter().map(|c| c.y).fold(f64::INFINITY, f64::min);
    assert!(
        (top - (400.0
            - corners.iter().map(|c| (*c - Vector2::splat(400.0)).length()).sum::<f64>() / 5.0))
            .abs()
            < 8.0
    );
}

#[test]
fn recognizes_a_hexagon_and_an_octagon_rather_than_circles() {
    for n in [6, 8] {
        let points = freehand(240, polygon_outline(drawn_regular_polygon(n, 150.0)));

        let corners = polygon_corners(recognize(&points, 50.0));

        assert_eq!(corners.len(), n);
        assert_regular(&corners);
    }
}

#[test]
fn recognizes_a_five_pointed_star() {
    let tips = drawn_regular_polygon(5, 160.0);
    let inner = drawn_regular_polygon(5, 65.0);
    // Around the star: a tip, then the inner corner between it and the next tip.
    let corners: Vec<Vector2> = (0..5)
        .flat_map(|i| {
            let between = Vector2::from_angle(-PI / 2.0 + 2.0 * PI * (i as f64 + 0.5) / 5.0);
            [
                tips[i],
                Vector2::new(400.0, 400.0) + between * (inner[i] - Vector2::splat(400.0)).length(),
            ]
        })
        .collect();
    let points = freehand(300, polygon_outline(corners));

    let corners = polygon_corners(recognize(&points, 50.0));

    assert_eq!(corners.len(), 10);
    assert_regular(&corners);
}

#[test]
fn recognizes_a_rhombus_as_a_polygon() {
    let diamond = vec![
        Vector2::new(400.0, 200.0),
        Vector2::new(520.0, 400.0),
        Vector2::new(400.0, 600.0),
        Vector2::new(280.0, 400.0),
    ];
    let points = freehand(200, polygon_outline(diamond));

    assert_eq!(polygon_corners(recognize(&points, 50.0)).len(), 4);
}

#[test]
fn recognizes_concave_shapes() {
    let l_shape = vec![
        Vector2::new(200.0, 200.0),
        Vector2::new(300.0, 200.0),
        Vector2::new(300.0, 450.0),
        Vector2::new(480.0, 450.0),
        Vector2::new(480.0, 550.0),
        Vector2::new(200.0, 550.0),
    ];
    let points = freehand(240, polygon_outline(l_shape));

    assert_eq!(polygon_corners(recognize(&points, 50.0)).len(), 6);
}

#[test]
fn recognizes_an_arrow_drawn_in_one_stroke() {
    // Shaft, then the head: out to one barb, back to the tip, out to the other.
    let vertices = vec![
        Vector2::new(100.0, 400.0),
        Vector2::new(500.0, 400.0),
        Vector2::new(430.0, 340.0),
        Vector2::new(500.0, 402.0),
        Vector2::new(432.0, 462.0),
    ];
    let points: Vec<Vector2> =
        (0..=200).map(|i| walk(&vertices, false, f64::from(i) / 200.0)).collect();

    let Some(Shape::Arrow(arrow)) = recognize(&points, 50.0) else {
        panic!("expected an arrow");
    };
    assert!((arrow.tip - Vector2::new(500.0, 400.0)).length() < 1.0, "{arrow:?}");
}

#[test]
fn recognizes_a_zigzag_as_a_polyline() {
    let vertices: Vec<Vector2> = (0..5)
        .map(|i| Vector2::new(100.0 + 80.0 * f64::from(i), if i % 2 == 0 { 300.0 } else { 420.0 }))
        .collect();
    let points: Vec<Vector2> =
        (0..=200).map(|i| walk(&vertices, false, f64::from(i) / 200.0)).collect();

    let Some(Shape::Polyline(polyline)) = recognize(&points, 50.0) else {
        panic!("expected a polyline");
    };
    assert_eq!(polyline.path.len(), 4);
}
