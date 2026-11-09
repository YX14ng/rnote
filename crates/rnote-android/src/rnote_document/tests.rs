//! Tests for [`RnoteDocument`](super::RnoteDocument), run on the host.

use parry2d_f64::math::Vector2;
use rnote_compose::Color;
use rnote_compose::builders::ShapeBuilderType;
use rnote_compose::penevent::KeyboardKey;
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;

use super::{EditState, RnoteDocument, TextEmphasis, ViewSize};
use crate::document_settings::TextFormat;
use crate::pen_input::{PenPhase, PenSample, Tool};
use crate::render::{BYTES_PER_PIXEL, DESK_COLOR, PixelLayout, Viewport};

const VIEW: ViewSize = ViewSize { width: 1000, height: 1000 };
const IDENTITY: Viewport = Viewport { x: 0.0, y: 0.0, zoom: 1.0 };
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

/// An empty document with a plain white background.
fn plain_document() -> RnoteDocument {
    let mut document = RnoteDocument::new_empty();
    let background = &mut document.engine.document.config.background;
    background.pattern = PatternStyle::None;
    background.color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    document
}

/// Builds the bytes of an empty document with a plain background of the given color.
fn empty_document_bytes(red: f64, green: f64, blue: f64) -> Vec<u8> {
    let mut document = plain_document();
    document.engine.document.config.background.color = Color { r: red, g: green, b: blue, a: 1.0 };
    document.save_to_bytes("test.rnote").expect("saving an empty document should succeed")
}

fn render_pixel(document: &mut RnoteDocument, x: f64, y: f64) -> [u8; 4] {
    let layout = PixelLayout { width: 1, height: 1, stride: BYTES_PER_PIXEL };
    let mut out = [0_u8; 4];
    document
        .render_rgba(Viewport { x, y, zoom: 1.0 }, layout, &mut out)
        .expect("rendering one pixel should succeed");
    out
}

fn to_byte(channel: f64) -> u8 {
    (channel * 255.0).round() as u8
}

fn assert_pixel_near(pixel: [u8; 4], expected: [u8; 4]) {
    for (channel, (got, want)) in pixel.iter().zip(expected).enumerate() {
        assert!(got.abs_diff(want) <= 1, "channel {channel}: got {pixel:?}, want {expected:?}");
    }
}

/// Draws a horizontal brush line at `y` from x = 100 to x = 300.
fn draw_line(document: &mut RnoteDocument, y: f64) -> EditState {
    let samples: Vec<PenSample> =
        (0..=20).map(|i| PenSample { x: 100.0 + 10.0 * f64::from(i), y, pressure: 1.0 }).collect();
    let (moves, last) = samples.split_at(samples.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW)
}

/// Counts the pixels of a `size` x `size` render at `zoom` that are darker than mid gray.
fn count_dark_pixels(document: &mut RnoteDocument, x: f64, y: f64, zoom: f64, size: u32) -> usize {
    let layout = PixelLayout { width: size, height: size, stride: size as usize * 4 };
    let mut out = vec![0_u8; layout.min_len()];
    document
        .render_rgba(Viewport { x, y, zoom }, layout, &mut out)
        .expect("rendering the region should succeed");
    out.as_chunks::<BYTES_PER_PIXEL>().0.iter().filter(|pixel| pixel[0] < 128).count()
}

#[test]
fn rejects_invalid_bytes() {
    assert!(RnoteDocument::try_open(b"not an rnote file".to_vec()).is_err());
}

#[test]
fn renders_background_inside_document() {
    let mut document = RnoteDocument::try_open(empty_document_bytes(1.0, 0.5, 0.0))
        .expect("an empty document should load");
    let center = document.bounds().center();

    let pixel = render_pixel(&mut document, center.x, center.y);

    assert_pixel_near(pixel, [to_byte(1.0), to_byte(0.5), to_byte(0.0), 255]);
}

#[test]
fn renders_desk_outside_document() {
    let mut document = RnoteDocument::try_open(empty_document_bytes(1.0, 1.0, 1.0))
        .expect("an empty document should load");
    let mins = document.bounds().mins;

    let pixel = render_pixel(&mut document, mins.x - 100.0, mins.y - 100.0);

    let [r, g, b] = DESK_COLOR;
    assert_pixel_near(pixel, [to_byte(r), to_byte(g), to_byte(b), 255]);
}

/// Darkest red channel in a 1 x 21 pixel column centered on the boundary between the
/// first two pages of a white, two page document.
fn darkest_at_page_boundary(show_borders: bool) -> u8 {
    let mut document = plain_document();
    let config = &mut document.engine.document.config;
    config.layout = Layout::FixedSize;
    config.format.show_borders = show_borders;
    config.format.border_color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let (page_width, page_height) = (config.format.width(), config.format.height());
    // An empty engine document sits wherever its (infinite layout) camera put it; pin it
    // to the origin, where the page grid is aligned.
    let doc = &mut document.engine.document;
    (doc.x, doc.y, doc.width, doc.height) = (0.0, 0.0, page_width, 2.0 * page_height);

    let layout = PixelLayout { width: 1, height: 21, stride: BYTES_PER_PIXEL };
    let mut out = vec![0_u8; layout.min_len()];
    let viewport = Viewport { x: 100.0, y: page_height - 10.0, zoom: 1.0 };
    document.render_rgba(viewport, layout, &mut out).expect("rendering should succeed");
    out.as_chunks::<BYTES_PER_PIXEL>().0.iter().map(|pixel| pixel[0]).min().unwrap_or(255)
}

#[test]
fn draws_page_borders_between_pages() {
    assert!(darkest_at_page_boundary(true) < 160, "the page border should be drawn");
    assert_eq!(darkest_at_page_boundary(false), 255, "no border when they are hidden");
}

#[test]
fn rejects_buffer_smaller_than_layout() {
    let mut document = plain_document();
    let layout = PixelLayout { width: 2, height: 2, stride: 2 * BYTES_PER_PIXEL };
    let mut out = vec![0_u8; layout.min_len() - 1];

    let result = document.render_rgba(IDENTITY, layout, &mut out);

    assert!(result.is_err(), "a short buffer must be rejected, not written past its end");
}

#[test]
fn renders_pattern_far_from_document_origin() {
    let mut document = plain_document();
    let background = &mut document.engine.document.config.background;
    background.pattern = PatternStyle::Grid;
    background.pattern_size = Vector2::new(4.0, 4.0);
    background.pattern_color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    // At zoom 40, y = 1000 is 40000 device pixels away from the document origin, past the
    // 16 bit range that pixman handles for pattern offsets.
    let near_origin = count_dark_pixels(&mut document, 100.0, 100.0, 40.0, 400);
    let far_from_origin = count_dark_pixels(&mut document, 100.0, 1000.0, 40.0, 400);

    assert!(near_origin > 0, "grid lines should be drawn near the origin");
    assert!(
        far_from_origin > 0,
        "grid lines vanished far from the origin (near origin: {near_origin} dark pixels)"
    );
}

#[test]
fn background_tiles_have_no_seams_when_zoomed_out() {
    let patterns = [
        PatternStyle::Lines,
        PatternStyle::Grid,
        PatternStyle::Dots,
        PatternStyle::IsometricGrid,
        PatternStyle::IsometricDots,
    ];
    for pattern in patterns {
        let mut document = plain_document();
        let background = &mut document.engine.document.config.background;
        background.pattern = pattern;
        background.pattern_color = Color { r: 0.8, g: 0.8, b: 0.8, a: 1.0 };
        let center = document.bounds().center();

        // Zooms where a tile is a fractional number of pixels wide.
        for zoom in [0.294, 0.5, 1.37, 3.629] {
            let layout = PixelLayout { width: 300, height: 300, stride: 300 * BYTES_PER_PIXEL };
            let mut out = vec![0_u8; layout.min_len()];
            document
                .render_rgba(Viewport { x: center.x, y: center.y, zoom }, layout, &mut out)
                .expect("rendering should succeed");
            let darkest = out.as_chunks::<BYTES_PER_PIXEL>().0.iter().map(|pixel| pixel[0]).min();

            // Nothing is darker than the light gray pattern: the desk must not show through.
            assert!(
                darkest >= Some(190),
                "seams with {pattern:?} at zoom {zoom}: darkest channel {darkest:?}"
            );
        }
    }
}

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

/// The text of every text stroke in the document.
fn texts(document: &RnoteDocument) -> Vec<String> {
    document
        .engine
        .extract_document_content()
        .strokes
        .iter()
        .filter_map(|layered| match layered.stroke.as_ref() {
            rnote_engine::strokes::Stroke::TextStroke(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn typewriter_types_and_edits_text() {
    let mut document = plain_document();
    document.set_brush(RED, 4.0);
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    let tapped = document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    assert!(tapped.text_input, "tapping with the text tool should ask for the keyboard");

    document.handle_text("Hola");
    document.handle_key(KeyboardKey::BackSpace, false, false);
    let state = document.handle_text("!");

    assert!(state.modified, "got {state:?}");
    assert_eq!(texts(&document), ["Hol!"]);
    let red_pixels = count_dark_pixels(&mut document, 80.0, 80.0, 1.0, 200);
    assert!(red_pixels > 0, "the text should be rendered");
}

#[test]
fn document_settings_change_page_layout_and_background() {
    let mut document = plain_document();
    document.set_layout(Layout::FixedSize);
    let state = document.set_page_size(148.0, 210.0);

    assert!(state.modified && state.bounds_changed, "got {state:?}");
    let settings = document.document_settings();
    assert!((settings.page_width_mm - 148.0).abs() < 0.01, "got {settings:?}");
    assert!(matches!(settings.layout, Layout::FixedSize));

    let blue = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    document.set_background(blue, PatternStyle::None, blue, 32.0);
    let center = document.bounds().center();
    assert_pixel_near(render_pixel(&mut document, center.x, center.y), [0, 0, 255, 255]);
}

#[test]
fn text_format_applies_to_the_text_being_edited() {
    let mut document = plain_document();
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_text("Hola");
    let small = count_dark_pixels(&mut document, 80.0, 60.0, 1.0, 300);

    let format = TextFormat {
        font_family: "serif".to_owned(),
        font_size: 64.0,
        bold: true,
        italic: false,
        alignment: rnote_engine::strokes::textstroke::TextAlignment::Start,
    };
    document.set_text_format(&format);
    let large = count_dark_pixels(&mut document, 80.0, 60.0, 1.0, 300);

    assert!(large > small * 2, "bigger bold text should cover more: {small} -> {large}");
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
fn saved_document_reopens_with_its_strokes() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);
    draw_line(&mut document, 200.0);

    let bytes = document.save_to_bytes("roundtrip.rnote").expect("saving should succeed");
    assert!(document.edit_state().modified, "serializing alone must not mark it as saved");
    document.mark_saved();

    assert!(!document.edit_state().modified, "mark_saved should clear the modified flag");
    let mut reopened = RnoteDocument::try_open(bytes).expect("the saved file should load");
    assert_pixel_near(render_pixel(&mut reopened, 200.0, 200.0), [255, 0, 0, 255]);
}

#[test]
fn copied_strokes_paste_into_another_document() {
    let mut source = plain_document();
    source.set_brush(RED, 6.0);
    draw_line(&mut source, 200.0);
    source.select_all();

    let entry = source.copy().expect("copying should succeed").expect("something is selected");
    assert_eq!(entry.mime_type, "application/rnote-stroke-content");

    let mut target = plain_document();
    let json = String::from_utf8(entry.data).expect("stroke content is JSON text");
    let state = target
        .paste_stroke_content(&json, Vector2::new(500.0, 500.0))
        .expect("pasting should succeed");

    assert!(state.modified && state.has_selection, "got {state:?}");
    // Leaving the selector deselects, so the selection overlay doesn't tint the stroke.
    target.set_tool(Tool::Brush);
    assert_pixel_near(render_pixel(&mut target, 500.0, 500.0), [255, 0, 0, 255]);
}

#[test]
fn cut_removes_the_selection() {
    let mut document = plain_document();
    document.set_brush(RED, 6.0);
    draw_line(&mut document, 200.0);
    document.select_all();

    let (entry, state) = document.cut().expect("cutting should succeed");

    assert!(entry.is_some() && state.modified, "got {entry:?} {state:?}");
    document.set_tool(Tool::Brush);
    assert_pixel_near(render_pixel(&mut document, 200.0, 200.0), [255, 255, 255, 255]);
}

#[test]
fn pasted_text_becomes_a_text_box() {
    let mut document = plain_document();

    let state = document.paste_text("Pegado", Vector2::new(100.0, 100.0));

    assert!(state.modified, "got {state:?}");
    assert_eq!(texts(&document), ["Pegado"]);
}

/// The ranged attributes of the first text stroke in the document.
fn text_attributes(document: &RnoteDocument) -> Vec<String> {
    document
        .engine
        .extract_document_content()
        .strokes
        .iter()
        .find_map(|layered| match layered.stroke.as_ref() {
            rnote_engine::strokes::Stroke::TextStroke(text) => Some(
                text.text_style
                    .ranged_text_attributes
                    .iter()
                    .map(|ranged| format!("{:?} {:?}", ranged.range, ranged.attribute))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn formats_only_the_selected_text() {
    let mut document = plain_document();
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_text("Hola mundo");
    // Shift+Home selects from the cursor back to the start of the line.
    document.handle_key(KeyboardKey::Home, true, false);

    document.toggle_text_emphasis(TextEmphasis::Bold);
    document.color_selected_text(RED);

    let attributes = text_attributes(&document);
    assert_eq!(attributes.len(), 2, "got {attributes:?}");
    assert!(attributes[0].starts_with("0..10 FontWeight(700)"), "got {attributes:?}");
    assert!(attributes[1].starts_with("0..10 TextColor"), "got {attributes:?}");
}

#[test]
fn ctrl_a_selects_all_the_text() {
    let mut document = plain_document();
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_text("abc");

    document.handle_key(KeyboardKey::Unicode('a'), false, true);
    document.toggle_text_emphasis(TextEmphasis::Italic);

    assert_eq!(texts(&document), ["abc"], "Ctrl+A must not type an 'a'");
    let attributes = text_attributes(&document);
    assert!(attributes.iter().any(|a| a.starts_with("0..3 Style(Italic)")), "got {attributes:?}");
}

#[test]
fn pattern_size_is_applied_and_clamped() {
    let mut document = plain_document();
    let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    document.set_background(white, PatternStyle::Grid, white, 64.0);
    assert_eq!(document.document_settings().pattern_size, 64.0);

    document.set_background(white, PatternStyle::Grid, white, 0.0);
    assert_eq!(document.document_settings().pattern_size, 4.0);
}

/// A narrow portrait view, like a tablet held upright, showing the origin page of a new
/// (infinite) document fitted to the page width, as `ViewportController` does.
fn portrait_origin_view(document: &RnoteDocument) -> (Viewport, ViewSize) {
    let view = ViewSize { width: 1848, height: 2700 };
    let zoom = f64::from(view.width) / document.document_settings().page_width;
    (Viewport { x: 0.0, y: 0.0, zoom }, view)
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
