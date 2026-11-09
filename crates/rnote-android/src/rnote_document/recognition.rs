//! Shape recognition for brush strokes: holding the pen still at the end of a stroke that
//! looks like a line, rectangle, ellipse or triangle replaces it with that shape, styled like
//! the shaper draws, like other note-taking apps do.

use std::time::Instant;

use parry2d_f64::math::Vector2;
use rnote_compose::PenEvent;
use rnote_compose::penpath::Element;
use rnote_engine::pens::PenMode;

use super::{EditState, RnoteDocument, ViewSize};
use crate::pen_input::{PenPhase, PenSample};
use crate::render::Viewport;
use crate::shape_recognition;

/// Whether recognition is on, and the stroke being drawn.
#[derive(Debug, Default)]
pub(super) struct Recognition {
    enabled: bool,
    /// Smallest size, in screen pixels, of strokes that may become shapes.
    min_size_px: f64,
    points: Vec<Vector2>,
    /// The current stroke became a shape: its remaining samples are dropped.
    snapped: bool,
}

impl RnoteDocument {
    /// Turns shape recognition for brush strokes on or off. Strokes smaller than
    /// `min_size_px` screen pixels are kept as drawn.
    pub fn set_shape_recognition(&mut self, enabled: bool, min_size_px: f64) -> EditState {
        self.recognition.enabled = enabled;
        self.recognition.min_size_px = min_size_px;
        self.recognition.points.clear();
        self.edit_state()
    }

    /// Replaces the brush stroke being drawn with the shape it looks like, if recognition is
    /// on and it looks like one, as a single undo step. Called while the pen holds still.
    ///
    /// When it does, [`EditState::shape_snapped`] is set and the rest of the stroke's
    /// samples, up to and including the pen lifting, are ignored. Otherwise the stroke goes
    /// on as if nothing happened.
    pub fn snap_to_shape(&mut self, viewport: Viewport, view_size: ViewSize) -> EditState {
        let unchanged = self.edit_state();
        let recognition = &self.recognition;
        if !recognition.enabled || recognition.snapped {
            return unchanged;
        }
        let min_size = recognition.min_size_px / viewport.zoom.max(f64::MIN_POSITIVE);
        let (Some(shape), Some(&last)) = (
            shape_recognition::recognize(&recognition.points, min_size),
            recognition.points.last(),
        ) else {
            return unchanged;
        };
        let bounds_before = self.bounds();
        let now = Instant::now();
        let mut flags = self.sync_camera(viewport, view_size);
        // End the stroke where the pen rests, then swap it for the shape: undoing removes the
        // freehand stroke, and recording the shape also drops it from the redo history.
        let up =
            PenEvent::Up { element: Element::new(last, 0.5), modifier_keys: Default::default() };
        let (_propagation, up_flags) = self.engine.handle_pen_event(up, Some(PenMode::Pen), now);
        flags |= up_flags;
        flags |= self.engine.undo(now);
        flags |= self.engine.insert_shapes(vec![shape]);
        self.recognition.points.clear();
        self.recognition.snapped = true;
        EditState { shape_snapped: true, ..self.finish_edit(flags, bounds_before) }
    }

    /// Follows the brush stroke being drawn. Returns false when the samples belong to a
    /// stroke that already became a shape, and must not reach the engine.
    pub(super) fn follow_brush_stroke(&mut self, phase: PenPhase, samples: &[PenSample]) -> bool {
        let recognition = &mut self.recognition;
        let ends = matches!(phase, PenPhase::Up | PenPhase::Cancel);
        if recognition.snapped {
            if ends {
                recognition.snapped = false;
            }
            return false;
        }
        if recognition.enabled && phase == PenPhase::Down {
            recognition
                .points
                .extend(samples.iter().map(|sample| Vector2::new(sample.x, sample.y)));
        }
        if ends {
            recognition.points.clear();
        }
        true
    }

    /// Forgets the stroke being followed, e.g. when another tool takes over.
    pub(super) fn reset_recognition(&mut self) {
        self.recognition.points.clear();
        self.recognition.snapped = false;
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use parry2d_f64::bounding_volume::Aabb;
    use rnote_engine::strokes::Stroke;

    use super::*;
    use crate::pen_input::Tool;

    const VIEW: ViewSize = ViewSize { width: 1000, height: 1000 };
    const IDENTITY: Viewport = Viewport { x: 0.0, y: 0.0, zoom: 1.0 };
    const MIN_SIZE_PX: f64 = 60.0;

    fn circle_samples(radius: f64) -> Vec<PenSample> {
        (0..=80)
            .map(|i| {
                let t = f64::from(i) / 80.0 * 2.0 * PI;
                PenSample {
                    x: 300.0 + t.cos() * radius,
                    y: 300.0 + t.sin() * radius,
                    pressure: 1.0,
                }
            })
            .collect()
    }

    fn pen_down(document: &mut RnoteDocument, samples: &[PenSample]) {
        document.handle_pen(PenPhase::Down, Tool::Brush, samples, IDENTITY, VIEW);
    }

    fn pen_up(document: &mut RnoteDocument, at: PenSample) -> EditState {
        document.handle_pen(PenPhase::Up, Tool::Brush, &[at], IDENTITY, VIEW)
    }

    fn strokes(document: &RnoteDocument) -> Vec<Stroke> {
        let everything = Aabb::new(Vector2::splat(-1e6), Vector2::splat(1e6));
        let content = document.engine.extract_content_in_bounds(everything);
        content.strokes.iter().map(|layered| (*layered.stroke).clone()).collect()
    }

    #[test]
    fn holding_still_turns_a_drawn_circle_into_a_shape_in_one_undo_step() {
        let mut document = RnoteDocument::new_empty();
        document.set_shape_recognition(true, MIN_SIZE_PX);
        let circle = circle_samples(100.0);
        pen_down(&mut document, &circle);

        let state = document.snap_to_shape(IDENTITY, VIEW);
        // The pen keeps moving a little, then lifts: none of it reaches the document.
        pen_down(&mut document, &circle[..10]);
        let after_lift = pen_up(&mut document, circle[10]);

        assert!(state.shape_snapped && state.can_undo && !state.can_redo, "got {state:?}");
        let strokes = strokes(&document);
        assert!(matches!(strokes.as_slice(), [Stroke::ShapeStroke(_)]), "got {strokes:?}");
        assert!(!after_lift.shape_snapped, "got {after_lift:?}");
        let undone = document.undo();
        assert!(self::strokes(&document).is_empty() && !undone.can_undo, "got {undone:?}");
    }

    #[test]
    fn lifting_without_holding_keeps_the_stroke() {
        let mut document = RnoteDocument::new_empty();
        document.set_shape_recognition(true, MIN_SIZE_PX);
        let circle = circle_samples(100.0);

        pen_down(&mut document, &circle[..80]);
        pen_up(&mut document, circle[80]);

        assert!(matches!(strokes(&document).as_slice(), [Stroke::BrushStroke(_)]));
    }

    #[test]
    fn holding_does_nothing_when_recognition_is_off() {
        let mut document = RnoteDocument::new_empty();
        let circle = circle_samples(100.0);
        pen_down(&mut document, &circle[..80]);

        let state = document.snap_to_shape(IDENTITY, VIEW);
        pen_up(&mut document, circle[80]);

        assert!(!state.shape_snapped);
        assert!(matches!(strokes(&document).as_slice(), [Stroke::BrushStroke(_)]));
    }

    #[test]
    fn strokes_that_are_no_shape_go_on_after_holding() {
        let mut document = RnoteDocument::new_empty();
        document.set_shape_recognition(true, MIN_SIZE_PX);
        let scribble: Vec<PenSample> = (0..60)
            .map(|i| {
                let t = f64::from(i);
                PenSample { x: 200.0 + t * 5.0, y: 300.0 + (t * 0.8).sin() * 80.0, pressure: 1.0 }
            })
            .collect();
        pen_down(&mut document, &scribble[..40]);

        let state = document.snap_to_shape(IDENTITY, VIEW);
        pen_down(&mut document, &scribble[40..59]);
        pen_up(&mut document, scribble[59]);

        assert!(!state.shape_snapped);
        assert!(matches!(strokes(&document).as_slice(), [Stroke::BrushStroke(_)]));
    }

    #[test]
    fn keeps_strokes_smaller_than_the_minimum_size() {
        let mut document = RnoteDocument::new_empty();
        document.set_shape_recognition(true, MIN_SIZE_PX);
        pen_down(&mut document, &circle_samples(20.0));

        assert!(!document.snap_to_shape(IDENTITY, VIEW).shape_snapped);
    }
}
