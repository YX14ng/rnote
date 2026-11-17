//! Conversion of stylus samples into engine pen events.

use std::collections::HashSet;
use std::time::{Duration, Instant};

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
    /// Draws shapes: lines, rectangles, ellipses, ...
    Shaper,
    /// Selects strokes to move, resize, rotate, duplicate or delete them.
    Selector,
    /// Places and edits text.
    Typewriter,
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
    /// The pen hovers over the surface without touching it.
    Proximity,
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
    /// When the pen was there, in milliseconds on any clock that only goes forward.
    pub time_ms: f64,
}

/// Gives the samples of a stroke the engine's instants, as far apart as they were taken.
///
/// The engine smooths a stroke with a model of the pen's motion, which needs to know when
/// the pen reached each position. Samples come a frame at a time; given the same instant,
/// the model draws straight lines from frame to frame.
#[derive(Debug, Default)]
pub(crate) struct PenClock {
    // A sample time and its instant, taken from the first batch of the stroke.
    anchor: Option<(Instant, f64)>,
}

impl PenClock {
    /// The stroke ended: the next one starts its own count.
    pub(crate) fn reset(&mut self) {
        self.anchor = None;
    }

    fn instant(&self, time_ms: f64, now: Instant) -> Instant {
        let Some((at, at_ms)) = self.anchor else {
            return now;
        };
        let offset =
            Duration::try_from_secs_f64((time_ms - at_ms).abs() / 1000.0).unwrap_or(Duration::ZERO);
        if time_ms >= at_ms { at + offset } else { at.checked_sub(offset).unwrap_or(at) }
    }
}

/// Builds the engine events for a batch of samples in the given phase, each with the
/// instant the pen was there; the newest sample of a stroke's first batch is `now`.
///
/// The engine expects a `Down` event for every position while the pen touches the surface,
/// and an `Up` event for the position where it is lifted.
pub(crate) fn pen_events(
    phase: PenPhase,
    samples: &[PenSample],
    clock: &mut PenClock,
    now: Instant,
) -> Vec<(PenEvent, Instant)> {
    if matches!(phase, PenPhase::Down | PenPhase::Up)
        && clock.anchor.is_none()
        && let Some(newest) = samples.last()
    {
        clock.anchor = Some((now, newest.time_ms));
    }
    let clock = &*clock;
    let down = |sample: &PenSample| {
        let event = PenEvent::Down { element: element(sample), modifier_keys: HashSet::new() };
        (event, clock.instant(sample.time_ms, now))
    };
    match phase {
        PenPhase::Down => samples.iter().map(down).collect(),
        PenPhase::Up => {
            let Some((last, moves)) = samples.split_last() else {
                return Vec::new();
            };
            let mut events: Vec<(PenEvent, Instant)> = moves.iter().map(down).collect();
            let up = PenEvent::Up { element: element(last), modifier_keys: HashSet::new() };
            events.push((up, clock.instant(last.time_ms, now)));
            events
        }
        PenPhase::Cancel => vec![(PenEvent::Cancel, now)],
        // Hovering draws nothing, so it needs no time of its own.
        PenPhase::Proximity => samples
            .iter()
            .map(|sample| {
                let event =
                    PenEvent::Proximity { element: element(sample), modifier_keys: HashSet::new() };
                (event, now)
            })
            .collect(),
    }
}

fn element(sample: &PenSample) -> Element {
    Element::new(Vector2::new(sample.x, sample.y), sample.pressure.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(x: f64, pressure: f64) -> PenSample {
        PenSample { x, y: 0.0, pressure, time_ms: 0.0 }
    }

    fn at(x: f64, time_ms: f64) -> PenSample {
        PenSample { x, y: 0.0, pressure: 0.5, time_ms }
    }

    /// The events of a stroke's first batch, without their instants.
    fn events(phase: PenPhase, samples: &[PenSample]) -> Vec<PenEvent> {
        let mut clock = PenClock::default();
        pen_events(phase, samples, &mut clock, Instant::now())
            .into_iter()
            .map(|(event, _)| event)
            .collect()
    }

    /// How long after `start` each event happens, in milliseconds.
    fn offsets(events: &[(PenEvent, Instant)], start: Instant) -> Vec<f64> {
        events.iter().map(|(_, at)| (*at - start).as_secs_f64() * 1000.0).collect()
    }

    #[test]
    fn samples_keep_the_time_between_them_across_batches() {
        let mut clock = PenClock::default();
        let start = Instant::now();

        let first =
            pen_events(PenPhase::Down, &[at(0.0, 100.0), at(1.0, 104.0)], &mut clock, start);
        // The next frame is handled 30 ms later, but its samples were taken 4 ms apart.
        let later = start + Duration::from_millis(30);
        let second = pen_events(PenPhase::Up, &[at(2.0, 108.0), at(3.0, 112.0)], &mut clock, later);

        let start = start - Duration::from_millis(4);
        let close = |got: Vec<f64>, want: [f64; 2]| {
            got.iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-6)
        };
        assert!(close(offsets(&first, start), [0.0, 4.0]), "{:?}", offsets(&first, start));
        assert!(close(offsets(&second, start), [8.0, 12.0]), "{:?}", offsets(&second, start));
    }

    #[test]
    fn a_new_stroke_starts_at_now_after_a_reset() {
        let mut clock = PenClock::default();
        let start = Instant::now();
        pen_events(PenPhase::Up, &[at(0.0, 100.0)], &mut clock, start);
        clock.reset();

        let later = start + Duration::from_secs(5);
        let next = pen_events(PenPhase::Down, &[at(0.0, 9000.0)], &mut clock, later);

        assert_eq!(next[0].1, later);
    }

    #[test]
    fn down_emits_one_down_event_per_sample() {
        let events = events(PenPhase::Down, &[sample(1.0, 0.5), sample(2.0, 0.5)]);

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| matches!(event, PenEvent::Down { .. })));
    }

    #[test]
    fn up_ends_with_an_up_event_at_the_last_sample() {
        let events = events(PenPhase::Up, &[sample(1.0, 0.5), sample(2.0, 0.5)]);

        assert!(matches!(events[0], PenEvent::Down { .. }));
        let PenEvent::Up { element, .. } = &events[1] else {
            panic!("the last event should be Up, got {:?}", events[1]);
        };
        assert_eq!(element.pos.x, 2.0);
    }

    #[test]
    fn up_without_samples_emits_nothing() {
        assert!(events(PenPhase::Up, &[]).is_empty());
    }

    #[test]
    fn cancel_emits_a_single_cancel() {
        let events = events(PenPhase::Cancel, &[sample(1.0, 0.5)]);

        assert!(matches!(events.as_slice(), [PenEvent::Cancel]));
    }

    #[test]
    fn proximity_emits_one_proximity_event_per_sample() {
        let events = events(PenPhase::Proximity, &[sample(1.0, 0.0), sample(2.0, 0.0)]);

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| matches!(event, PenEvent::Proximity { .. })));
    }

    #[test]
    fn clamps_pressure() {
        let events = events(PenPhase::Down, &[sample(0.0, 1.7), sample(0.0, -0.2)]);
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
