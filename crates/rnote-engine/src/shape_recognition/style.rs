//! How a recognized shape looks: like the stroke that was drawn, as far as the shaper allows.

use rnote_compose::Style;
use rnote_compose::shapes::Shape;

use crate::pens::pensconfig::PensConfig;

/// The style of a recognized `shape`: the shaper's (smooth or rough, filled or not) with the
/// color and width of the brush in use, so it looks like the stroke it replaces. Open shapes
/// (lines, polylines, arrows, curves) are never filled: a filled bracket would be a half moon.
pub fn recognized_style(pens: &PensConfig, shape: &Shape) -> Style {
    let (color, width) = match pens.brush_config.style_for_current_options() {
        Style::Smooth(options) => (options.stroke_color, options.stroke_width),
        Style::Rough(options) => (options.stroke_color, options.stroke_width),
        Style::Textured(options) => (options.stroke_color, options.stroke_width),
    };
    let open = matches!(
        shape,
        Shape::Line(_)
            | Shape::Polyline(_)
            | Shape::Arrow(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
    );
    let mut style = pens.shaper_config.gen_style_for_current_options();
    match &mut style {
        Style::Smooth(options) => {
            options.stroke_color = color;
            options.stroke_width = width;
            if open {
                options.fill_color = None;
            }
        }
        Style::Rough(options) => {
            options.stroke_color = color;
            options.stroke_width = width;
            if open {
                options.fill_color = None;
            }
        }
        Style::Textured(_) => {}
    }
    style
}
