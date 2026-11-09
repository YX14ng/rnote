//! Shape recognition for brush strokes: a finished stroke that looks like a line, rectangle,
//! ellipse or triangle is replaced by that shape, styled like the shaper draws.

use std::time::Instant;

use parry2d_f64::math::Vector2;
use rnote_engine::WidgetFlags;

use super::{EditState, RnoteDocument};
use crate::pen_input::{PenPhase, PenSample};
use crate::shape_recognition;

/// Whether recognition is on, and the stroke being drawn.
#[derive(Debug, Default)]
pub(super) struct Recognition {
    enabled: bool,
    /// Smallest size, in screen pixels, of strokes that may become shapes.
    min_size_px: f64,
    points: Vec<Vector2>,
}

impl RnoteDocument {
    /// Turns shape recognition for brush strokes on or off. Strokes smaller than
    /// `min_size_px` screen pixels are kept as drawn, so handwriting stays handwriting.
    pub fn set_shape_recognition(&mut self, enabled: bool, min_size_px: f64) -> EditState {
        self.recognition.enabled = enabled;
        self.recognition.min_size_px = min_size_px;
        self.recognition.points.clear();
        self.edit_state()
    }

    /// Follows the brush stroke being drawn. Once it ends, replaces it with the shape it looks
    /// like, if recognition is on and it looks like one, as a single undo step.
    pub(super) fn recognize_brush_stroke(
        &mut self,
        phase: PenPhase,
        samples: &[PenSample],
        zoom: f64,
    ) -> WidgetFlags {
        let mut flags = WidgetFlags::default();
        if !self.recognition.enabled {
            return flags;
        }
        let points = &mut self.recognition.points;
        points.extend(samples.iter().map(|sample| Vector2::new(sample.x, sample.y)));
        match phase {
            PenPhase::Down | PenPhase::Proximity => return flags,
            PenPhase::Cancel => {}
            PenPhase::Up => {
                let min_size = self.recognition.min_size_px / zoom.max(f64::MIN_POSITIVE);
                if let Some(shape) = shape_recognition::recognize(points, min_size)
                    && self.engine.can_undo()
                {
                    let now = Instant::now();
                    // Undoing removes the freehand stroke; the shape is then recorded in
                    // its place, which also drops the stroke from the redo history.
                    flags |= self.engine.undo(now);
                    flags |= self.engine.insert_shapes(vec![shape]);
                }
            }
        }
        self.recognition.points.clear();
        flags
    }

    /// Forgets the stroke being followed, e.g. when another tool takes over.
    pub(super) fn reset_recognition(&mut self) {
        self.recognition.points.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use parry2d_f64::bounding_volume::Aabb;
    use rnote_engine::strokes::Stroke;

    use super::*;
    use crate::pen_input::Tool;
    use crate::render::Viewport;
    use crate::rnote_document::ViewSize;

    const VIEW: ViewSize = ViewSize { width: 1000, height: 1000 };
    const IDENTITY: Viewport = Viewport { x: 0.0, y: 0.0, zoom: 1.0 };
    const MIN_SIZE_PX: f64 = 60.0;

    /// Draws a closed loop of `radius` around (300, 300) with the brush.
    fn draw_circle(document: &mut RnoteDocument, radius: f64) -> EditState {
        let samples: Vec<PenSample> = (0..=80)
            .map(|i| {
                let t = f64::from(i) / 80.0 * 2.0 * PI;
                PenSample {
                    x: 300.0 + t.cos() * radius,
                    y: 300.0 + t.sin() * radius,
                    pressure: 1.0,
                }
            })
            .collect();
        let (moves, last) = samples.split_at(samples.len() - 1);
        document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
        document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW)
    }

    fn strokes(document: &RnoteDocument) -> Vec<Stroke> {
        let everything = Aabb::new(Vector2::splat(-1e6), Vector2::splat(1e6));
        let content = document.engine.extract_content_in_bounds(everything);
        content.strokes.iter().map(|layered| (*layered.stroke).clone()).collect()
    }

    #[test]
    fn replaces_a_drawn_circle_with_a_shape_in_one_undo_step() {
        let mut document = RnoteDocument::new_empty();
        document.set_shape_recognition(true, MIN_SIZE_PX);

        let state = draw_circle(&mut document, 100.0);

        let strokes = strokes(&document);
        assert!(matches!(strokes.as_slice(), [Stroke::ShapeStroke(_)]), "got {strokes:?}");
        assert!(state.modified && state.can_undo && !state.can_redo, "got {state:?}");
        let state = document.undo();
        assert!(strokes_are_empty(&document) && !state.can_undo, "got {state:?}");
    }

    #[test]
    fn keeps_strokes_when_recognition_is_off() {
        let mut document = RnoteDocument::new_empty();

        draw_circle(&mut document, 100.0);

        assert!(matches!(strokes(&document).as_slice(), [Stroke::BrushStroke(_)]));
    }

    #[test]
    fn keeps_handwriting_sized_strokes() {
        let mut document = RnoteDocument::new_empty();
        document.set_shape_recognition(true, MIN_SIZE_PX);

        // An "o" smaller than the minimum size on screen.
        draw_circle(&mut document, 20.0);

        assert!(matches!(strokes(&document).as_slice(), [Stroke::BrushStroke(_)]));
    }

    fn strokes_are_empty(document: &RnoteDocument) -> bool {
        strokes(document).is_empty()
    }
}
