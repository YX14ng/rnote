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
