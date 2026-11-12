//! Document settings, saving, and copying between documents.

use super::*;

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
fn pattern_size_is_applied_and_clamped() {
    let mut document = plain_document();
    let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    document.set_background(white, PatternStyle::Grid, white, 64.0);
    assert_eq!(document.document_settings().pattern_size, 64.0);

    document.set_background(white, PatternStyle::Grid, white, 0.0);
    assert_eq!(document.document_settings().pattern_size, 4.0);
}

#[test]
fn saved_document_reopens_where_it_was_viewed() {
    let mut document = plain_document();
    let layout = PixelLayout { width: 10, height: 10, stride: 10 * BYTES_PER_PIXEL };
    let mut out = vec![0_u8; 10 * 10 * BYTES_PER_PIXEL];
    document
        .render_rgba(Viewport { x: 40.0, y: 1500.0, zoom: 2.5 }, layout, &mut out)
        .expect("rendering should succeed");

    let bytes = document.save_to_bytes("viewed.rnote").expect("saving should succeed");
    let reopened = RnoteDocument::try_open(bytes).expect("the saved file should load");

    let origin = reopened.saved_view_origin();
    assert!((origin.x - 40.0).abs() < 1e-6, "x was {}", origin.x);
    assert!((origin.y - 1500.0).abs() < 1e-6, "y was {}", origin.y);
}
