//! Holding the pen still for a moment during a brush stroke turns the stroke into the shape
//! it looks like, as on Android.

use std::time::{Duration, Instant};

use gtk4::{glib, glib::clone, prelude::*, subclass::prelude::*};
use p2d::math::Vector2;
use rnote_compose::penevent::PenState;
use rnote_engine::pens::{PenMode, PenStyle};
use rnote_engine::shape_recognition::StrokeTracker;

use super::RnCanvas;
use crate::RnAppWindow;

/// How long the pen must hold still.
const HOLD: Duration = Duration::from_millis(1000);
/// How far, in screen pixels, the resting pen may wander and still count as holding still.
const SLOP_PX: f64 = 6.0;
/// The smallest a stroke may be, in screen pixels, to become a shape.
const MIN_SIZE_PX: f64 = 24.0;

/// The brush stroke being followed and the wait for the pen to hold still.
#[derive(Debug, Default)]
pub(crate) struct ShapeHold {
    tracker: StrokeTracker,
    /// Where the pen started holding still, in document coordinates.
    anchor: Option<Vector2>,
    timer: Option<glib::SourceId>,
}

impl ShapeHold {
    fn stop_waiting(&mut self) {
        self.anchor = None;
        if let Some(timer) = self.timer.take() {
            timer.remove();
        }
    }
}

/// Whether an element in `pen_state` ends the stroke being followed: any with the pen up,
/// not only the release, which may go missing (e.g. the focus changed with the pen down).
fn ends_stroke(pen_state: PenState) -> bool {
    pen_state != PenState::Down
}

/// Whether the pen moved from `anchor` to `point` (document coordinates) farther than it may
/// while holding still, at `zoom`.
fn moved_enough(anchor: Vector2, point: Vector2, zoom: f64) -> bool {
    (point - anchor).length() * zoom > SLOP_PX
}

impl RnCanvas {
    /// Follows a pen element on its way to the engine. Returns false when it belongs to a
    /// stroke that already became a shape, and must not reach the engine.
    pub(super) fn shape_hold_follow(
        &self,
        pen_state: PenState,
        pen_mode: Option<PenMode>,
        pos: Vector2,
    ) -> bool {
        let drawing = pen_state == PenState::Down
            && pen_mode != Some(PenMode::Eraser)
            && self.engine_ref().current_pen_style_w_override() == PenStyle::Brush
            && self.recognizes_shapes();
        let ends = ends_stroke(pen_state);
        let keep = self
            .imp()
            .shape_hold
            .borrow_mut()
            .tracker
            .follow(drawing, ends, [pos]);
        if drawing && keep {
            self.wait_for_hold(pos);
        } else {
            self.imp().shape_hold.borrow_mut().stop_waiting();
        }
        keep
    }

    /// Whether "Recognize Shapes" is on in the settings.
    fn recognizes_shapes(&self) -> bool {
        self.root()
            .and_downcast::<RnAppWindow>()
            .is_some_and(|appwindow| {
                appwindow
                    .sidebar()
                    .settings_panel()
                    .general_shape_recognition_row()
                    .is_active()
            })
    }

    /// The pen is at `pos`: starts waiting again if it moved away from where it rested.
    fn wait_for_hold(&self, pos: Vector2) {
        let zoom = self.engine_ref().camera.total_zoom();
        let mut hold = self.imp().shape_hold.borrow_mut();
        if hold
            .anchor
            .is_some_and(|anchor| !moved_enough(anchor, pos, zoom))
        {
            return;
        }
        hold.stop_waiting();
        hold.anchor = Some(pos);
        hold.timer = Some(glib::timeout_add_local_once(
            HOLD,
            clone!(
                #[weak(rename_to = canvas)]
                self,
                move || canvas.snap_held_stroke()
            ),
        ));
    }

    /// The pen held still: the stroke becomes the shape it looks like, if any.
    fn snap_held_stroke(&self) {
        let zoom = self.engine_ref().camera.total_zoom();
        let flags = {
            let mut hold = self.imp().shape_hold.borrow_mut();
            // The timer ran: there is nothing to remove anymore.
            hold.timer = None;
            let mut engine = self.engine_mut();
            hold.tracker
                .snap(&mut engine, MIN_SIZE_PX / zoom, Instant::now())
        };
        if let Some(flags) = flags {
            self.emit_handle_widget_flags(flags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_event_with_the_pen_up_ends_the_stroke() {
        assert!(ends_stroke(PenState::Up));
        // The release went missing (focus changed with the pen down): hovering ends it too.
        assert!(ends_stroke(PenState::Proximity));
        assert!(!ends_stroke(PenState::Down));
    }

    #[test]
    fn moving_counts_in_screen_pixels() {
        let anchor = Vector2::new(100.0, 100.0);

        assert!(!moved_enough(anchor, Vector2::new(105.0, 100.0), 1.0));
        assert!(moved_enough(anchor, Vector2::new(107.0, 100.0), 1.0));
        // Zoomed in twice, 4 units of the page are 8 pixels of the screen.
        assert!(moved_enough(anchor, Vector2::new(100.0, 104.0), 2.0));
    }
}
