//! Following a brush stroke and swapping it for the shape it looks like.

use std::f64::consts::PI;
use std::time::Instant;

use p2d::math::Vector2;
use rnote_compose::PenEvent;
use rnote_compose::penpath::Element;

use super::StrokeTracker;
use crate::Engine;
use crate::pens::PenMode;
use crate::strokes::Stroke;

const MIN_SIZE: f64 = 60.0;

fn circle(radius: f64) -> Vec<Vector2> {
    (0..=80)
        .map(|i| {
            let t = f64::from(i) / 80.0 * 2.0 * PI;
            Vector2::new(300.0 + t.cos() * radius, 300.0 + t.sin() * radius)
        })
        .collect()
}

/// Sends `points` as brush samples with the pen down, as a frontend does, skipping those
/// the tracker says belong to a stroke already turned into a shape.
fn draw(engine: &mut Engine, tracker: &mut StrokeTracker, points: &[Vector2]) {
    for &point in points {
        if tracker.follow(true, false, [point]) {
            let down = PenEvent::Down {
                element: Element::new(point, 0.5),
                modifier_keys: Default::default(),
            };
            let _ = engine.handle_pen_event(down, Some(PenMode::Pen), Instant::now());
        }
    }
}

fn lift(engine: &mut Engine, tracker: &mut StrokeTracker, at: Vector2) {
    if tracker.follow(false, true, [at]) {
        let up = PenEvent::Up {
            element: Element::new(at, 0.5),
            modifier_keys: Default::default(),
        };
        let _ = engine.handle_pen_event(up, Some(PenMode::Pen), Instant::now());
    }
}

fn strokes(engine: &Engine) -> Vec<Stroke> {
    engine
        .store
        .stroke_keys_as_rendered()
        .into_iter()
        .filter_map(|key| engine.store.get_stroke_ref(key).cloned())
        .collect()
}

#[test]
fn holding_turns_a_circle_into_one_shape_in_one_undo_step() {
    let mut engine = Engine::default();
    let mut tracker = StrokeTracker::default();
    let circle = circle(100.0);
    draw(&mut engine, &mut tracker, &circle);

    assert!(
        tracker
            .snap(&mut engine, MIN_SIZE, Instant::now())
            .is_some()
    );
    lift(&mut engine, &mut tracker, circle[80]);

    assert!(matches!(
        strokes(&engine).as_slice(),
        [Stroke::ShapeStroke(_)]
    ));
    assert!(engine.can_undo() && !engine.can_redo());
    let _ = engine.undo(Instant::now());
    assert!(strokes(&engine).is_empty());
    assert!(!engine.can_undo());
}

#[test]
fn samples_after_snapping_are_dropped_until_the_pen_lifts() {
    let mut engine = Engine::default();
    let mut tracker = StrokeTracker::default();
    let circle = circle(100.0);
    draw(&mut engine, &mut tracker, &circle);
    let _ = tracker.snap(&mut engine, MIN_SIZE, Instant::now());

    // The pen wanders on before lifting: none of it reaches the engine.
    draw(&mut engine, &mut tracker, &circle[..20]);
    lift(&mut engine, &mut tracker, circle[20]);
    assert!(matches!(
        strokes(&engine).as_slice(),
        [Stroke::ShapeStroke(_)]
    ));

    // The next stroke is drawn as usual.
    draw(&mut engine, &mut tracker, &circle[..40]);
    lift(&mut engine, &mut tracker, circle[40]);
    assert_eq!(strokes(&engine).len(), 2);
}

#[test]
fn no_shape_leaves_the_stroke_going() {
    let mut engine = Engine::default();
    let mut tracker = StrokeTracker::default();
    let scribble: Vec<Vector2> = (0..60)
        .map(|i| {
            let t = f64::from(i);
            Vector2::new(200.0 + t * 5.0, 300.0 + (t * 0.8).sin() * 80.0)
        })
        .collect();
    draw(&mut engine, &mut tracker, &scribble[..40]);

    assert!(
        tracker
            .snap(&mut engine, MIN_SIZE, Instant::now())
            .is_none()
    );
    draw(&mut engine, &mut tracker, &scribble[40..59]);
    lift(&mut engine, &mut tracker, scribble[59]);

    assert!(matches!(
        strokes(&engine).as_slice(),
        [Stroke::BrushStroke(_)]
    ));
}

#[test]
fn small_strokes_are_left_alone() {
    let mut engine = Engine::default();
    let mut tracker = StrokeTracker::default();
    draw(&mut engine, &mut tracker, &circle(20.0));

    assert!(
        tracker
            .snap(&mut engine, MIN_SIZE, Instant::now())
            .is_none()
    );
}

/// The shape of the only stroke, as inserted.
fn only_shape(engine: &Engine) -> crate::strokes::ShapeStroke {
    match strokes(engine).as_slice() {
        [Stroke::ShapeStroke(shape)] => shape.clone(),
        other => panic!("expected one shape, got {other:?}"),
    }
}

#[test]
fn recognized_shapes_take_the_brush_color_and_width() {
    use rnote_compose::shapes::{QuadraticBezier, Rectangle, Shape};
    use rnote_compose::{Color, Style};

    let blue = Color::new(0.0, 0.0, 1.0, 1.0);
    let red = Color::new(1.0, 0.0, 0.0, 1.0);
    let green = Color::new(0.0, 1.0, 0.0, 1.0);
    let make_engine = || {
        let engine = Engine::default();
        {
            let mut config = engine.config.write();
            let brush = &mut config.pens_config.brush_config;
            brush.style = crate::pens::pensconfig::brushconfig::BrushStyle::Solid;
            brush.solid_options.stroke_color = Some(blue);
            brush.solid_options.stroke_width = 3.0;
            let shaper = &mut config.pens_config.shaper_config;
            shaper.smooth_options.stroke_color = Some(red);
            shaper.smooth_options.stroke_width = 9.0;
            shaper.smooth_options.fill_color = Some(green);
        }
        engine
    };
    let rectangle = Shape::Rectangle(Rectangle::from_corners(
        Vector2::new(0.0, 0.0),
        Vector2::new(100.0, 50.0),
    ));
    let bracket = Shape::QuadraticBezier(QuadraticBezier {
        start: Vector2::new(0.0, 0.0),
        cp: Vector2::new(-30.0, 100.0),
        end: Vector2::new(0.0, 200.0),
    });

    let mut engine = make_engine();
    let _ = engine.insert_recognized_shape(rectangle);
    let Style::Smooth(options) = only_shape(&engine).style else {
        panic!("expected a smooth style");
    };
    assert_eq!(options.stroke_color, Some(blue));
    assert_eq!(options.stroke_width, 3.0);
    assert_eq!(options.fill_color, Some(green));

    let mut engine = make_engine();
    let _ = engine.insert_recognized_shape(bracket);
    let Style::Smooth(options) = only_shape(&engine).style else {
        panic!("expected a smooth style");
    };
    assert_eq!(options.stroke_color, Some(blue));
    assert_eq!(options.fill_color, None);
}

#[test]
fn a_stroke_the_brush_never_started_is_left_alone() {
    let mut engine = Engine::default();
    let mut tracker = StrokeTracker::default();
    let earlier = circle(50.0);
    draw(&mut engine, &mut tracker, &earlier);
    lift(&mut engine, &mut tracker, earlier[80]);

    // Samples the brush never saw, e.g. off the page: no ink, so nothing to swap, and the
    // earlier stroke must not be undone.
    let _ = tracker.follow(true, false, circle(100.0));
    assert!(
        tracker
            .snap(&mut engine, MIN_SIZE, Instant::now())
            .is_none()
    );

    assert!(matches!(
        strokes(&engine).as_slice(),
        [Stroke::BrushStroke(_)]
    ));
}
