//! Setting the paper of a document: page size, how it grows, color and template.

use parry2d_f64::math::Vector2;
use rnote_compose::Color;
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::{Engine, WidgetFlags};

const MM_PER_INCH: f64 = 25.4;

/// Smallest and largest pattern spacing offered, in document units.
const PATTERN_SIZE_RANGE: (f64, f64) = (4.0, 512.0);

/// The color stored as ARGB (`0xAARRGGBB`), as Android and the library keep colors.
#[must_use]
pub fn color_from_argb(argb: u32) -> Color {
    let [alpha, red, green, blue] = argb.to_be_bytes();
    let channel = |value: u8| f64::from(value) / 255.0;
    Color { r: channel(red), g: channel(green), b: channel(blue), a: channel(alpha) }
}

/// Changes the page size, keeping the document's resolution.
pub fn apply_page_size(engine: &mut Engine, width_mm: f64, height_mm: f64) -> WidgetFlags {
    let format = &mut engine.document.config.format;
    let to_pixels = |mm: f64| mm / MM_PER_INCH * format.dpi();
    let (width, height) = (to_pixels(width_mm), to_pixels(height_mm));
    format.set_width(width);
    format.set_height(height);
    // Like the desktop app: the pages follow the new format, content stays where it is.
    engine.doc_resize_to_fit_content()
}

/// Changes how the document grows.
pub fn apply_layout(engine: &mut Engine, layout: Layout) -> WidgetFlags {
    engine.set_doc_layout(layout)
}

/// Changes the background color and pattern; `pattern_size` is the spacing in both directions.
pub fn apply_background(
    engine: &mut Engine,
    color: Color,
    pattern: PatternStyle,
    pattern_color: Color,
    pattern_size: f64,
) {
    let background = &mut engine.document.config.background;
    background.color = color;
    background.pattern = pattern;
    background.pattern_color = pattern_color;
    let (min, max) = PATTERN_SIZE_RANGE;
    background.pattern_size = Vector2::splat(pattern_size.clamp(min, max));
}
