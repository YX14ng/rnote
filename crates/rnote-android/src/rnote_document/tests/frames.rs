//! Frames after small changes redraw only where something changed, and look the same as
//! frames drawn whole.

use super::*;
use crate::render::BYTES_PER_PIXEL;

const VIEW_SIZE: (u32, u32) = (500, 400);
const AT: Viewport = Viewport { x: 0.0, y: 0.0, zoom: 1.0 };

/// A page of short lines in rows, on a grid background.
fn written() -> RnoteDocument {
    let mut document = plain_document();
    document.engine.document.config.background.pattern = PatternStyle::Grid;
    document.set_brush(Color::BLACK, 3.0);
    for row in 0..8 {
        let y = 30.0 + 45.0 * f64::from(row);
        let samples: Vec<PenSample> = (0..=20)
            .map(|i| PenSample {
                x: 20.0 + 22.0 * f64::from(i),
                y: y + (f64::from(i) * 0.7).sin() * 8.0,
                pressure: 0.8,
                time_ms: 4.0 * f64::from(i),
            })
            .collect();
        let (moves, last) = samples.split_at(samples.len() - 1);
        document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
        document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW);
    }
    // A margin line down across every row: any region repainted has ink of it to redraw.
    let margin: Vec<PenSample> = (0..=40)
        .map(|i| PenSample {
            x: 100.0,
            y: 10.0 + 9.0 * f64::from(i),
            pressure: 0.8,
            time_ms: 4.0 * f64::from(i),
        })
        .collect();
    let (moves, last) = margin.split_at(margin.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW);
    document
}

fn layout() -> PixelLayout {
    PixelLayout {
        width: VIEW_SIZE.0,
        height: VIEW_SIZE.1,
        stride: VIEW_SIZE.0 as usize * BYTES_PER_PIXEL,
    }
}

/// A frame as the canvas gets it, from the kept frame when it can.
fn frame(document: &mut RnoteDocument, viewport: Viewport) -> Vec<u8> {
    let mut out = vec![0_u8; layout().min_len()];
    document.render_rgba(viewport, layout(), &mut out).expect("renders");
    out
}

/// The same frame drawn whole.
fn whole(document: &mut RnoteDocument, viewport: Viewport) -> Vec<u8> {
    let mut out = vec![0_u8; layout().min_len()];
    document.render_rgba_whole(viewport, layout(), &mut out).expect("renders");
    out
}

fn assert_same(got: &[u8], want: &[u8]) {
    let worst = got.iter().zip(want).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    assert!(worst <= 1, "pixels differ by up to {worst}");
}

fn pen(document: &mut RnoteDocument, tool: Tool, points: &[(f64, f64)]) {
    let samples: Vec<PenSample> = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| PenSample { x, y, pressure: 1.0, time_ms: 4.0 * i as f64 })
        .collect();
    let (moves, last) = samples.split_at(samples.len() - 1);
    document.handle_pen(PenPhase::Down, tool, moves, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, tool, last, IDENTITY, VIEW);
}

#[test]
fn erasing_redraws_only_around_what_went_and_looks_the_same() {
    let mut document = written();
    frame(&mut document, AT);

    // Down across the second row of lines, with a pen's dense samples.
    let across: Vec<(f64, f64)> = (0..15).map(|i| (205.0, 50.0 + 4.0 * f64::from(i))).collect();
    pen(&mut document, Tool::Eraser, &across);
    let partial = frame(&mut document, AT);

    assert_eq!(document.frames.partial_frames(), 1);
    assert_same(&partial, &whole(&mut document, AT));
}

#[test]
fn moving_a_selection_looks_the_same_as_a_whole_frame() {
    let mut document = written();
    pen(
        &mut document,
        Tool::Selector,
        &[(10.0, 15.0), (480.0, 15.0), (480.0, 55.0), (10.0, 55.0), (10.0, 15.0)],
    );
    frame(&mut document, AT);

    pen(&mut document, Tool::Selector, &[(100.0, 30.0), (110.0, 40.0), (120.0, 200.0)]);
    let partial = frame(&mut document, AT);

    assert_same(&partial, &whole(&mut document, AT));
}

#[test]
fn undo_looks_the_same_as_a_whole_frame() {
    let mut document = written();
    frame(&mut document, AT);
    document.undo();

    let partial = frame(&mut document, AT);

    assert_eq!(document.frames.partial_frames(), 1);
    assert_same(&partial, &whole(&mut document, AT));
}

#[test]
fn another_view_is_drawn_whole() {
    let mut document = written();
    frame(&mut document, AT);

    frame(&mut document, Viewport { y: 30.0, ..AT });

    assert_eq!(document.frames.partial_frames(), 0);
}
