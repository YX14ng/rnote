//! Pens: brush, eraser, shaper and selection, and the camera they draw with.

use std::f64::consts::PI;

use parry2d_f64::bounding_volume::Aabb;
use rnote_compose::penpath::Segment;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::strokes::Stroke;

use super::*;

#[test]
fn brush_draws_a_stroke() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);

    let state = draw_line(&mut document, 200.0);

    assert!(state.modified && state.can_undo && !state.can_redo, "got {state:?}");
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 0, 0, 255]);
}

#[test]
fn undo_and_redo_remove_and_restore_the_stroke() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);
    draw_line(&mut document, 200.0);

    let undone = document.undo();
    assert!(undone.can_redo, "got {undone:?}");
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);

    document.redo();
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 0, 0, 255]);
}

#[test]
fn eraser_removes_the_stroke() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);
    draw_line(&mut document, 200.0);

    let across = [
        PenSample { x: 200.0, y: 150.0, pressure: 1.0, time_ms: 0.0 },
        PenSample { x: 200.0, y: 200.0, pressure: 1.0, time_ms: 0.0 },
    ];
    document.handle_pen(PenPhase::Down, Tool::Eraser, &across, IDENTITY, VIEW);
    let end = [PenSample { x: 200.0, y: 250.0, pressure: 1.0, time_ms: 0.0 }];
    document.handle_pen(PenPhase::Up, Tool::Eraser, &end, IDENTITY, VIEW);

    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);
}

#[test]
fn shaper_draws_a_rectangle_outline() {
    let mut document = plain_document();
    document.set_brush(RED, 4.0);
    document.set_shape(ShapeBuilderType::Rectangle);
    let corner = |x, y| PenSample { x, y, pressure: 1.0, time_ms: 0.0 };
    let drag = [corner(100.0, 100.0), corner(200.0, 200.0), corner(300.0, 300.0)];
    document.handle_pen(PenPhase::Down, Tool::Shaper, &drag, IDENTITY, VIEW);
    let state =
        document.handle_pen(PenPhase::Up, Tool::Shaper, &[corner(300.0, 300.0)], IDENTITY, VIEW);

    assert!(state.modified && state.can_undo, "got {state:?}");
    assert_pixel_near(render_pixel(&mut document, 100.0, 200.0), [255, 0, 0, 255]);
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);
}

#[test]
fn shaper_works_after_erasing_with_the_eraser_end() {
    let mut document = plain_document();
    document.set_brush(RED, 4.0);
    let point = |x, y| PenSample { x, y, pressure: 1.0, time_ms: 0.0 };
    // The stylus' eraser end leaves the engine in eraser mode.
    document.handle_pen(PenPhase::Up, Tool::Eraser, &[point(500.0, 500.0)], IDENTITY, VIEW);
    document.set_tool(Tool::Shaper);
    document.set_shape(ShapeBuilderType::Rectangle);

    let drag = [point(100.0, 100.0), point(300.0, 300.0)];
    document.handle_pen(PenPhase::Down, Tool::Shaper, &drag, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Shaper, &[point(300.0, 300.0)], IDENTITY, VIEW);

    assert_pixel_near(render_pixel(&mut document, 100.0, 200.0), [255, 0, 0, 255]);
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);
}

#[test]
fn polyline_takes_several_taps_and_finishes_with_enter() {
    let mut document = plain_document();
    document.set_brush(RED, 4.0);
    document.set_shape(ShapeBuilderType::Polyline);
    let point = |x, y| PenSample { x, y, pressure: 1.0, time_ms: 0.0 };
    for corner in [point(100.0, 100.0), point(300.0, 100.0), point(300.0, 300.0)] {
        document.handle_pen(PenPhase::Down, Tool::Shaper, &[corner], IDENTITY, VIEW);
        document.handle_pen(PenPhase::Up, Tool::Shaper, &[corner], IDENTITY, VIEW);
    }
    assert!(document.engine.extract_document_content().strokes.is_empty(), "not finished yet");

    let state = document.handle_key(KeyboardKey::CarriageReturn, false, false);

    assert!(state.modified && state.can_undo, "got {state:?}");
    assert_pixel_near(render_pixel(&mut document, 200.0, 100.0), [255, 0, 0, 255]);
    assert_pixel_near(render_pixel(&mut document, 300.0, 200.0), [255, 0, 0, 255]);
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);
}

#[test]
fn deletes_and_restores_the_selection() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);
    draw_line(&mut document, 200.0);

    let selected = document.select_all();
    assert!(selected.has_selection, "got {selected:?}");
    let deleted = document.delete_selection();
    assert!(!deleted.has_selection, "got {deleted:?}");
    document.set_tool(Tool::Brush);
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);

    document.undo();
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 0, 0, 255]);
}

#[test]
fn infinite_document_keeps_its_size_while_drawing_on_the_origin_page() {
    let mut document = RnoteDocument::new_empty();
    let (viewport, view) = portrait_origin_view(&document);
    let bounds_before = document.bounds();

    for screen_y in [300.0, 1350.0, 2400.0] {
        let line: Vec<PenSample> = (0..=20)
            .map(|i| PenSample {
                x: viewport.x + (300.0 + 45.0 * f64::from(i)) / viewport.zoom,
                y: viewport.y + screen_y / viewport.zoom,
                pressure: 1.0,
                time_ms: 0.0,
            })
            .collect();
        let (moves, last) = line.split_at(line.len() - 1);
        document.handle_pen(PenPhase::Down, Tool::Brush, moves, viewport, view);
        document.handle_pen(PenPhase::Up, Tool::Brush, last, viewport, view);
    }

    // Fitting the view to the whole document instead made it grow on every stroke.
    assert_eq!(document.bounds(), bounds_before);
}

#[test]
fn eraser_works_at_the_bottom_of_a_portrait_view() {
    let mut document = RnoteDocument::new_empty();
    document.set_brush(Color::BLACK, 6.0);
    let (viewport, view) = portrait_origin_view(&document);
    let at = |screen_x: f64, screen_y: f64| PenSample {
        x: viewport.x + screen_x / viewport.zoom,
        y: viewport.y + screen_y / viewport.zoom,
        pressure: 1.0,
        time_ms: 0.0,
    };
    let line: Vec<PenSample> = (0..=20).map(|i| at(300.0 + 45.0 * f64::from(i), 2600.0)).collect();
    let (moves, last) = line.split_at(line.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Brush, moves, viewport, view);
    document.handle_pen(PenPhase::Up, Tool::Brush, last, viewport, view);
    let crossing = at(750.0, 2600.0);
    let dark_before =
        count_dark_pixels(&mut document, crossing.x - 32.0, crossing.y - 32.0, 1.0, 64);

    let across: Vec<PenSample> =
        (0..=30).map(|i| at(750.0, 2450.0 + 10.0 * f64::from(i))).collect();
    let (moves, last) = across.split_at(across.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Eraser, moves, viewport, view);
    document.handle_pen(PenPhase::Up, Tool::Eraser, last, viewport, view);

    assert!(dark_before > 0, "the line should have been drawn");
    assert_eq!(count_dark_pixels(&mut document, crossing.x - 32.0, crossing.y - 32.0, 1.0, 64), 0);
}

#[test]
fn engine_camera_matches_a_centered_view_of_a_small_document() {
    let mut document = plain_document();
    let config = &mut document.engine.document.config;
    config.layout = Layout::FixedSize;
    let (page_width, page_height) = (config.format.width(), config.format.height());
    let doc = &mut document.engine.document;
    (doc.x, doc.y, doc.width, doc.height) = (0.0, 0.0, page_width, page_height);
    // Zoomed out so the page is centered with plenty of desk around it, farther out than
    // the engine would scroll by itself.
    let zoom = 0.25;
    let viewport = Viewport { x: -(1000.0 / zoom - page_width) / 2.0, y: -500.0, zoom };

    let _flags = document.sync_camera(viewport, VIEW);

    let camera_view = document.engine.camera.viewport();
    assert!((camera_view.mins.x - viewport.x).abs() < 1e-6, "got {camera_view:?}");
    assert!((camera_view.mins.y - viewport.y).abs() < 1e-6, "got {camera_view:?}");
}

/// Drags the selector around the loop through `corners`, as a lasso.
fn lasso(document: &mut RnoteDocument, corners: &[(f64, f64)]) -> EditState {
    document.set_tool(Tool::Selector);
    let mut samples: Vec<PenSample> = Vec::new();
    for (i, &(x, y)) in corners.iter().enumerate() {
        let (next_x, next_y) = corners[(i + 1) % corners.len()];
        samples.extend((0..10).map(|step| {
            let t = f64::from(step) / 10.0;
            PenSample {
                x: x + (next_x - x) * t,
                y: y + (next_y - y) * t,
                pressure: 1.0,
                time_ms: 0.0,
            }
        }));
    }
    let (moves, last) = samples.split_at(samples.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Selector, moves, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Selector, last, IDENTITY, VIEW)
}

#[test]
fn selector_selects_what_a_lasso_encloses() {
    let mut document = plain_document();
    draw_line(&mut document, 200.0);

    let around =
        lasso(&mut document, &[(80.0, 150.0), (330.0, 140.0), (320.0, 260.0), (90.0, 250.0)]);

    assert!(around.has_selection, "got {around:?}");
}

#[test]
fn lasso_elsewhere_selects_nothing() {
    let mut document = plain_document();
    draw_line(&mut document, 200.0);

    let elsewhere =
        lasso(&mut document, &[(80.0, 400.0), (330.0, 400.0), (320.0, 500.0), (90.0, 500.0)]);

    assert!(!elsewhere.has_selection, "got {elsewhere:?}");
}

#[test]
fn selection_bounds_follow_the_selected_strokes_and_recolor_them() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);
    draw_line(&mut document, 200.0);
    assert!(document.selection_bounds().is_none());

    lasso(&mut document, &[(80.0, 150.0), (330.0, 140.0), (320.0, 260.0), (90.0, 250.0)]);
    let bounds = document.selection_bounds().expect("the line should be selected");
    let state = document.recolor_selection(Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 });
    document.deselect_all();

    assert!(bounds.mins.x <= 100.0 && bounds.maxs.x >= 300.0, "got {bounds:?}");
    assert!(bounds.mins.y <= 200.0 && bounds.maxs.y >= 200.0, "got {bounds:?}");
    assert!(state.modified && state.can_undo, "got {state:?}");
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [0, 0, 255, 255]);
}

/// `StrokeWidth` in the app's `StrokePreview.kt` mirrors these curves to draw the stroke
/// being written as wide as the engine will.
#[test]
fn brush_styles_keep_the_pressure_curves_the_preview_assumes() {
    use rnote_compose::style::PressureCurve;
    use rnote_engine::pens::pensconfig::BrushConfig;

    let config = BrushConfig::default();

    assert!(matches!(config.marker_options.pressure_curve, PressureCurve::Const));
    assert!(matches!(config.solid_options.pressure_curve, PressureCurve::Linear));
    assert!(matches!(config.textured_options.pressure_curve, PressureCurve::Linear));
}

/// Draws a horizontal line at `y` pressing lightly.
fn draw_light_line(document: &mut RnoteDocument, y: f64) {
    let samples: Vec<PenSample> = (0..=20)
        .map(|i| PenSample { x: 100.0 + 10.0 * f64::from(i), y, pressure: 0.2, time_ms: 0.0 })
        .collect();
    let (moves, last) = samples.split_at(samples.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW);
}

#[test]
fn a_ballpoint_keeps_its_width_whatever_the_pressure() {
    let mut pen = plain_document();
    pen.set_brush(RED, 12.0);
    pen.set_brush_style(BrushStyle::Solid);
    let mut ballpoint = plain_document();
    ballpoint.set_brush(RED, 12.0);
    ballpoint.set_brush_style(BrushStyle::Solid);
    ballpoint.set_constant_width(true);

    draw_light_line(&mut pen, 200.0);
    draw_light_line(&mut ballpoint, 200.0);

    // 5 units off the middle: inside a 12 wide line, outside a 12 × 0.2 one.
    assert_pixel_near(render_pixel(&mut pen, 200.0, 205.0), [255, 255, 255, 255]);
    assert_pixel_near(render_pixel(&mut ballpoint, 200.0, 205.0), [255, 0, 0, 255]);
}

/// How far the brush stroke's path strays from the circle of `radius` around (`cx`, `cy`).
fn farthest_from_circle(document: &RnoteDocument, cx: f64, cy: f64, radius: f64) -> f64 {
    let everything = Aabb::new(Vector2::splat(-1e6), Vector2::splat(1e6));
    let content = document.engine.extract_content_in_bounds(everything);
    let [layered] = content.strokes.as_slice() else {
        panic!("expected one stroke, got {}", content.strokes.len());
    };
    let Stroke::BrushStroke(brush) = &*layered.stroke else {
        panic!("expected a brush stroke");
    };
    let ends = brush.path.segments.iter().map(|segment| match segment {
        Segment::LineTo { end } => end.pos,
        Segment::QuadBezTo { end, .. } | Segment::CubBezTo { end, .. } => end.pos,
    });
    ends.map(|pos| ((pos - Vector2::new(cx, cy)).length() - radius).abs()).fold(0.0, f64::max)
}

#[test]
fn brush_keeps_a_curve_round_when_samples_come_in_batches() {
    let mut document = plain_document();
    document.set_brush(Color::BLACK, 2.0);
    // A stylus reports every 4 ms, but the app gets them a frame at a time: 6 per batch.
    let samples: Vec<PenSample> = (0..=180)
        .map(|i| {
            let angle = f64::from(i) / 180.0 * 1.5 * PI;
            PenSample {
                x: 500.0 + 100.0 * angle.cos(),
                y: 500.0 + 100.0 * angle.sin(),
                pressure: 1.0,
                time_ms: 4.0 * f64::from(i),
            }
        })
        .collect();
    let (moves, last) = samples.split_at(samples.len() - 1);
    for batch in moves.chunks(6) {
        document.handle_pen(PenPhase::Down, Tool::Brush, batch, IDENTITY, VIEW);
    }
    document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW);

    let off = farthest_from_circle(&document, 500.0, 500.0, 100.0);
    assert!(off < 1.0, "the stroke strays {off:.2} from the circle");
}

#[test]
fn a_brush_stroke_in_progress_is_left_to_the_apps_preview() {
    let mut document = plain_document();
    document.set_brush(Color::BLACK, 6.0);
    let samples: Vec<PenSample> = (0..=20)
        .map(|i| PenSample {
            x: 100.0 + 10.0 * f64::from(i),
            y: 200.0,
            pressure: 1.0,
            time_ms: 4.0 * f64::from(i),
        })
        .collect();

    document.handle_pen(PenPhase::Down, Tool::Brush, &samples, IDENTITY, VIEW);

    // The app draws the stroke being written itself, sooner; the frame would show it twice.
    assert_eq!(count_dark_pixels(&mut document, 0.0, 0.0, 1.0, 400), 0);
    document.handle_pen(PenPhase::Up, Tool::Brush, &samples[20..], IDENTITY, VIEW);
    assert!(count_dark_pixels(&mut document, 0.0, 0.0, 1.0, 400) > 0);
}

#[test]
fn the_app_draws_the_lasso_being_drawn_and_hears_about_it() {
    let mut document = plain_document();
    document.set_brush(Color::BLACK, 4.0);
    draw_line(&mut document, 200.0);
    let corner = |x, y| PenSample { x, y, pressure: 1.0, time_ms: 0.0 };
    // Away from the line: only the lasso would show.
    let state = document.handle_pen(
        PenPhase::Down,
        Tool::Selector,
        &[corner(20.0, 300.0), corner(380.0, 300.0), corner(380.0, 380.0), corner(20.0, 380.0)],
        IDENTITY,
        VIEW,
    );

    assert!(state.lasso, "got {state:?}");
    assert_eq!(count_dark_pixels(&mut document, 0.0, 250.0, 1.0, 150), 0);
}

#[test]
fn the_app_draws_the_eraser_while_it_rubs() {
    let mut document = plain_document();
    let at = |x, y| PenSample { x, y, pressure: 1.0, time_ms: 0.0 };

    document.handle_pen(
        PenPhase::Down,
        Tool::Eraser,
        &[at(200.0, 300.0), at(210.0, 300.0)],
        IDENTITY,
        VIEW,
    );

    // The engine's red square would be there; nothing else is.
    let pixel = render_pixel(&mut document, 210.0, 300.0);
    assert_pixel_near(pixel, [255, 255, 255, 255]);
}
