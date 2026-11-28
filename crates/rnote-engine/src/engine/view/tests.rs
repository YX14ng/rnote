//! Restoring a view after the document was replaced underneath it.

use p2d::math::Vector2;
use rnote_compose::Style;
use rnote_compose::shapes::{Line, Shape};

use crate::Engine;
use crate::store::render_comp::RenderCompState;
use crate::strokes::{ShapeStroke, Stroke};

#[test]
fn strokes_in_the_restored_view_get_drawn() {
    let mut engine = Engine::default();
    let _ = engine.camera.set_size(Vector2::new(800.0, 600.0));
    let far = Vector2::new(5000.0, 5000.0);
    let line = Line {
        start: far,
        end: far + Vector2::new(200.0, 0.0),
    };
    let key = engine.store.insert_stroke(
        Stroke::ShapeStroke(ShapeStroke::new(Shape::Line(line), Style::default())),
        None,
    );
    // Shown at the origin, as the file said: the far stroke isn't drawn yet.
    let _ = engine.update_rendering_current_viewport();
    assert_eq!(
        engine.store.render_comp_state(key),
        Some(RenderCompState::Dirty)
    );

    let _ = engine.restore_view(
        far - Vector2::new(100.0, 100.0),
        1.0,
        Vector2::new(800.0, 600.0),
    );

    assert_ne!(
        engine.store.render_comp_state(key),
        Some(RenderCompState::Dirty)
    );
}

#[test]
fn the_view_comes_back_with_this_screens_size_not_the_other_devices() {
    let mut engine = Engine::default();
    let far = Vector2::new(100.0, 6000.0);
    let line = Line {
        start: far,
        end: far + Vector2::new(200.0, 0.0),
    };
    let _ = engine.store.insert_stroke(
        Stroke::ShapeStroke(ShapeStroke::new(Shape::Line(line), Style::default())),
        None,
    );
    // The file came from a tablet held upright: its view is much taller than this one.
    let _ = engine.camera.set_size(Vector2::new(800.0, 2560.0));

    let offset = Vector2::new(0.0, 5800.0);
    let _ = engine.restore_view(offset, 1.0, Vector2::new(800.0, 600.0));

    assert_eq!(engine.camera.size(), Vector2::new(800.0, 600.0));
    assert_eq!(engine.camera.offset(), offset);
}
