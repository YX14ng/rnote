//! Conversion of stylus samples into engine pen events.

use std::collections::HashSet;

use parry2d_f64::math::Vector2;
use rnote_compose::PenEvent;
use rnote_compose::penpath::Element;

/// The tool a pen interaction uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Draws strokes with the brush.
    Brush,
    /// Erases strokes.
    Eraser,
}

/// Where a batch of samples sits in a pen interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenPhase {
    /// The pen touches the surface: it was just pressed or it moved.
    Down,
    /// The pen was lifted; the last sample is where it left the surface.
    Up,
    /// The interaction was aborted, e.g. because the system took over the touch.
    Cancel,
}

/// One stylus sample in document coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenSample {
    /// Horizontal document coordinate.
    pub x: f64,
    /// Vertical document coordinate.
    pub y: f64,
    /// Pressure in `0.0..=1.0`; values outside are clamped.
    pub pressure: f64,
}

/// Builds the engine events for a batch of samples in the given phase.
///
/// The engine expects a `Down` event for every position while the pen touches the surface,
/// and an `Up` event for the position where it is lifted.
pub(crate) fn pen_events(phase: PenPhase, samples: &[PenSample]) -> Vec<PenEvent> {
    let down = |sample: &PenSample| PenEvent::Down {
        element: element(sample),
        modifier_keys: HashSet::new(),
    };
    match phase {
        PenPhase::Down => samples.iter().map(down).collect(),
        PenPhase::Up => {
            let Some((last, moves)) = samples.split_last() else {
                return Vec::new();
            };
            let mut events: Vec<PenEvent> = moves.iter().map(down).collect();
            events.push(PenEvent::Up { element: element(last), modifier_keys: HashSet::new() });
            events
        }
        PenPhase::Cancel => vec![PenEvent::Cancel],
    }
}

fn element(sample: &PenSample) -> Element {
    Element::new(Vector2::new(sample.x, sample.y), sample.pressure.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(x: f64, pressure: f64) -> PenSample {
        PenSample { x, y: 0.0, pressure }
    }

    #[test]
    fn down_emits_one_down_event_per_sample() {
        let events = pen_events(PenPhase::Down, &[sample(1.0, 0.5), sample(2.0, 0.5)]);

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| matches!(event, PenEvent::Down { .. })));
    }

    #[test]
    fn up_ends_with_an_up_event_at_the_last_sample() {
        let events = pen_events(PenPhase::Up, &[sample(1.0, 0.5), sample(2.0, 0.5)]);

        assert!(matches!(events[0], PenEvent::Down { .. }));
        let PenEvent::Up { element, .. } = &events[1] else {
            panic!("the last event should be Up, got {:?}", events[1]);
        };
        assert_eq!(element.pos.x, 2.0);
    }

    #[test]
    fn up_without_samples_emits_nothing() {
        assert!(pen_events(PenPhase::Up, &[]).is_empty());
    }

    #[test]
    fn cancel_emits_a_single_cancel() {
        let events = pen_events(PenPhase::Cancel, &[sample(1.0, 0.5)]);

        assert!(matches!(events.as_slice(), [PenEvent::Cancel]));
    }

    #[test]
    fn clamps_pressure() {
        let events = pen_events(PenPhase::Down, &[sample(0.0, 1.7), sample(0.0, -0.2)]);
        let pressures: Vec<f64> = events
            .iter()
            .map(|event| match event {
                PenEvent::Down { element, .. } => element.pressure,
                other => panic!("expected Down, got {other:?}"),
            })
            .collect();

        assert_eq!(pressures, [1.0, 0.0]);
    }
}
