//! Following the brush stroke being drawn, to swap it for the shape it looks like.
//!
//! A frontend passes every brush sample through [`StrokeTracker::follow`] before handing it
//! to the engine, and calls [`StrokeTracker::snap`] when the pen holds still. Once a stroke
//! became a shape, the rest of its samples, up to the pen lifting, must not reach the engine.

use std::time::Instant;

use p2d::math::Vector2;
use rnote_compose::PenEvent;
use rnote_compose::penpath::Element;

use crate::pens::{Pen, PenMode};
use crate::{Engine, WidgetFlags};

/// The points of the brush stroke being drawn, and whether it already became a shape.
#[derive(Debug, Default, Clone)]
pub struct StrokeTracker {
    points: Vec<Vector2>,
    snapped: bool,
}

impl StrokeTracker {
    /// Follows samples of the brush stroke: `drawing` when the pen is down drawing them,
    /// `ends` when the stroke ends or is cancelled with them. Returns false when they belong
    /// to a stroke that already became a shape, and must not reach the engine.
    pub fn follow(
        &mut self,
        drawing: bool,
        ends: bool,
        points: impl IntoIterator<Item = Vector2>,
    ) -> bool {
        if self.snapped {
            if ends {
                self.snapped = false;
            }
            return false;
        }
        if drawing {
            self.points.extend(points);
        }
        if ends {
            self.points.clear();
        }
        true
    }

    /// Forgets the stroke being followed, e.g. when another tool takes over.
    pub fn reset(&mut self) {
        self.points.clear();
        self.snapped = false;
    }

    /// Whether the stroke being drawn looks like a shape at least `min_size` (document units)
    /// along some axis, and hasn't become one yet.
    pub fn looks_like_a_shape(&self, min_size: f64) -> bool {
        !self.snapped && super::recognize(&self.points, min_size).is_some()
    }

    /// Swaps the stroke being drawn for the shape it looks like, as a single undo step, if
    /// it looks like one and is at least `min_size` (document units) along some axis.
    /// `None` when it doesn't, or already became a shape.
    pub fn snap(
        &mut self,
        engine: &mut Engine,
        min_size: f64,
        now: Instant,
    ) -> Option<WidgetFlags> {
        if self.snapped {
            return None;
        }
        // Without ink there is nothing to swap, and undoing would take back an earlier
        // action instead: the brush starts no stroke off the page, for one.
        if !matches!(engine.penholder.current_pen_ref(), Pen::Brush(brush) if brush.is_drawing()) {
            return None;
        }
        let last = *self.points.last()?;
        let shape = super::recognize(&self.points, min_size)?;
        // End the stroke where the pen rests, then swap it for the shape: undoing removes the
        // freehand stroke, and recording the shape also drops it from the redo history.
        let up = PenEvent::Up {
            element: Element::new(last, 0.5),
            modifier_keys: Default::default(),
        };
        let (_propagation, mut flags) = engine.handle_pen_event(up, Some(PenMode::Pen), now);
        flags |= engine.undo(now);
        flags |= engine.insert_recognized_shape(shape);
        self.points.clear();
        self.snapped = true;
        Some(flags)
    }
}

#[cfg(test)]
#[path = "tracker_tests.rs"]
mod tests;
