//! Rendering: background, pattern, page borders and the desk.

use super::*;

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
