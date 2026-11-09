//! Pens: brush, eraser, shaper and selection, and the camera they draw with.

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
        PenSample { x: 200.0, y: 150.0, pressure: 1.0 },
        PenSample { x: 200.0, y: 200.0, pressure: 1.0 },
    ];
    document.handle_pen(PenPhase::Down, Tool::Eraser, &across, IDENTITY, VIEW);
    let end = [PenSample { x: 200.0, y: 250.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Up, Tool::Eraser, &end, IDENTITY, VIEW);

    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);
}

#[test]
fn shaper_draws_a_rectangle_outline() {
    let mut document = plain_document();
    document.set_brush(RED, 4.0);
    document.set_shape(ShapeBuilderType::Rectangle);
    let corner = |x, y| PenSample { x, y, pressure: 1.0 };
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
    let point = |x, y| PenSample { x, y, pressure: 1.0 };
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
    let point = |x, y| PenSample { x, y, pressure: 1.0 };
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
