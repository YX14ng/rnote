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

pub(super) const VIEW: ViewSize = ViewSize { width: 1000, height: 1000 };
pub(super) const IDENTITY: Viewport = Viewport { x: 0.0, y: 0.0, zoom: 1.0 };
pub(super) const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

/// An empty document with a plain white background.
pub(super) fn plain_document() -> RnoteDocument {
    let mut document = RnoteDocument::new_empty();
    let background = &mut document.engine.document.config.background;
    background.pattern = PatternStyle::None;
    background.color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    document
}

/// Builds the bytes of an empty document with a plain background of the given color.
pub(super) fn empty_document_bytes(red: f64, green: f64, blue: f64) -> Vec<u8> {
    let mut document = plain_document();
    document.engine.document.config.background.color = Color { r: red, g: green, b: blue, a: 1.0 };
    document.save_to_bytes("test.rnote").expect("saving an empty document should succeed")
}

pub(super) fn render_pixel(document: &mut RnoteDocument, x: f64, y: f64) -> [u8; 4] {
    let layout = PixelLayout { width: 1, height: 1, stride: BYTES_PER_PIXEL };
    let mut out = [0_u8; 4];
    document
        .render_rgba(Viewport { x, y, zoom: 1.0 }, layout, &mut out)
        .expect("rendering one pixel should succeed");
    out
}

pub(super) fn to_byte(channel: f64) -> u8 {
    (channel * 255.0).round() as u8
}

pub(super) fn assert_pixel_near(pixel: [u8; 4], expected: [u8; 4]) {
    for (channel, (got, want)) in pixel.iter().zip(expected).enumerate() {
        assert!(got.abs_diff(want) <= 1, "channel {channel}: got {pixel:?}, want {expected:?}");
    }
}

/// Draws a horizontal brush line at `y` from x = 100 to x = 300.
pub(super) fn draw_line(document: &mut RnoteDocument, y: f64) -> EditState {
    let samples: Vec<PenSample> =
        (0..=20).map(|i| PenSample { x: 100.0 + 10.0 * f64::from(i), y, pressure: 1.0 }).collect();
    let (moves, last) = samples.split_at(samples.len() - 1);
    document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW)
}

/// Counts the pixels of a `size` x `size` render at `zoom` that are darker than mid gray.
pub(super) fn count_dark_pixels(
    document: &mut RnoteDocument,
    x: f64,
    y: f64,
    zoom: f64,
    size: u32,
) -> usize {
    let layout = PixelLayout { width: size, height: size, stride: size as usize * 4 };
    let mut out = vec![0_u8; layout.min_len()];
    document
        .render_rgba(Viewport { x, y, zoom }, layout, &mut out)
        .expect("rendering the region should succeed");
    out.as_chunks::<BYTES_PER_PIXEL>().0.iter().filter(|pixel| pixel[0] < 128).count()
}

/// The text of every text stroke in the document.
pub(super) fn texts(document: &RnoteDocument) -> Vec<String> {
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

/// The ranged attributes of the first text stroke in the document.
pub(super) fn text_attributes(document: &RnoteDocument) -> Vec<String> {
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

/// A narrow portrait view, like a tablet held upright, showing the origin page of a new
/// (infinite) document fitted to the page width, as `ViewportController` does.
pub(super) fn portrait_origin_view(document: &RnoteDocument) -> (Viewport, ViewSize) {
    let view = ViewSize { width: 1848, height: 2700 };
    let zoom = f64::from(view.width) / document.document_settings().page_width;
    (Viewport { x: 0.0, y: 0.0, zoom }, view)
}

mod documents;
mod pens;
mod rendering;
mod text;
