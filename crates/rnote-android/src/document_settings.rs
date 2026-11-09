//! Text format, shape style and document settings: what the formatting menus change.

use parry2d_f64::math::Vector2;
use rnote_compose::Color;
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::engine::EngineConfigShared;
use rnote_engine::pens::pensconfig::shaperconfig::ShaperStyle;
use rnote_engine::strokes::textstroke::{FontStyle, TextAlignment, TextStyle};
use rnote_engine::{Engine, WidgetFlags};

/// Font weight of bold text (CSS scale).
const BOLD_WEIGHT: u16 = 700;
const MM_PER_INCH: f64 = 25.4;

/// How text looks.
#[derive(Debug, Clone)]
pub struct TextFormat {
    /// Font family name; generic names (`serif`, `sans-serif`, `monospace`, `cursive`) keep
    /// documents portable between devices.
    pub font_family: String,
    /// Font size in document units.
    pub font_size: f64,
    /// Bold weight instead of the regular one.
    pub bold: bool,
    /// Italic style.
    pub italic: bool,
    /// Alignment of the lines in the text box.
    pub alignment: TextAlignment,
}

impl TextFormat {
    fn apply_to(&self, style: &mut TextStyle) {
        style.font_family.clone_from(&self.font_family);
        style.font_size = self.font_size.clamp(TextStyle::FONT_SIZE_MIN, TextStyle::FONT_SIZE_MAX);
        style.font_weight = if self.bold { BOLD_WEIGHT } else { TextStyle::FONT_WEIGHT_DEFAULT };
        style.font_style = if self.italic { FontStyle::Italic } else { FontStyle::Regular };
        style.alignment = self.alignment;
    }
}

/// How shapes are drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeStyle {
    /// Hand-drawn look instead of exact lines.
    pub rough: bool,
    /// Fill of closed shapes, or none.
    pub fill: Option<Color>,
}

/// The document settings the menus show and change.
#[derive(Debug, Clone, Copy)]
pub struct DocumentSettings {
    /// Page width in millimeters.
    pub page_width_mm: f64,
    /// Page height in millimeters.
    pub page_height_mm: f64,
    /// How the document grows.
    pub layout: Layout,
    /// Background color.
    pub background_color: Color,
    /// Background pattern.
    pub pattern: PatternStyle,
    /// Color of the background pattern.
    pub pattern_color: Color,
    /// Spacing of the background pattern, in document units.
    pub pattern_size: f64,
    /// Whether page outlines are drawn.
    pub show_borders: bool,
}

/// Sets the format of new text, and of the text being edited if any.
pub(crate) fn apply_text_format(
    engine: &mut Engine,
    config: &EngineConfigShared,
    format: &TextFormat,
) -> WidgetFlags {
    format.apply_to(&mut config.write().pens_config.typewriter_config.text_style);
    engine.text_selection_change_style(|style| format.apply_to(style))
}

/// Sets the color of the text being edited, if any. New text gets it from the pens config.
pub(crate) fn apply_text_color(engine: &mut Engine, color: Color) -> WidgetFlags {
    engine.text_selection_change_style(|style| style.color = color)
}

/// Sets the style of new shapes.
pub(crate) fn apply_shape_style(config: &EngineConfigShared, style: ShapeStyle) {
    let mut config = config.write();
    let shaper = &mut config.pens_config.shaper_config;
    shaper.style = if style.rough { ShaperStyle::Rough } else { ShaperStyle::Smooth };
    shaper.smooth_options.fill_color = style.fill;
    shaper.rough_options.fill_color = style.fill;
}

/// The current document settings.
pub(crate) fn document_settings(engine: &Engine) -> DocumentSettings {
    let config = &engine.document.config;
    let format = &config.format;
    let to_mm = |pixels: f64| pixels / format.dpi() * MM_PER_INCH;
    DocumentSettings {
        page_width_mm: to_mm(format.width()),
        page_height_mm: to_mm(format.height()),
        layout: config.layout,
        background_color: config.background.color,
        pattern: config.background.pattern,
        pattern_color: config.background.pattern_color,
        pattern_size: config.background.pattern_size.y,
        show_borders: format.show_borders,
    }
}

/// Changes the page size, keeping the document's resolution.
pub(crate) fn apply_page_size(engine: &mut Engine, width_mm: f64, height_mm: f64) -> WidgetFlags {
    let format = &mut engine.document.config.format;
    let to_pixels = |mm: f64| mm / MM_PER_INCH * format.dpi();
    let (width, height) = (to_pixels(width_mm), to_pixels(height_mm));
    format.set_width(width);
    format.set_height(height);
    // Like the desktop app: the pages follow the new format, content stays where it is.
    engine.doc_resize_to_fit_content()
}

/// Changes how the document grows.
pub(crate) fn apply_layout(engine: &mut Engine, layout: Layout) -> WidgetFlags {
    engine.set_doc_layout(layout)
}

/// Smallest and largest pattern spacing offered, in document units.
const PATTERN_SIZE_RANGE: (f64, f64) = (4.0, 512.0);

/// Changes the background color and pattern; `pattern_size` is the spacing in both directions.
pub(crate) fn apply_background(
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

/// Shows or hides the page outlines.
pub(crate) fn apply_page_borders(engine: &mut Engine, show: bool) {
    engine.document.config.format.show_borders = show;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_round_trips_through_millimeters() {
        let mut engine = Engine::default();

        let _flags = apply_page_size(&mut engine, 210.0, 297.0);
        let settings = document_settings(&engine);

        assert!((settings.page_width_mm - 210.0).abs() < 0.01, "got {settings:?}");
        assert!((settings.page_height_mm - 297.0).abs() < 0.01, "got {settings:?}");
    }

    #[test]
    fn text_format_sets_the_style_of_new_text() {
        let mut engine = Engine::default();
        let config = EngineConfigShared::default();
        let _flags = engine.install_config(&config, None);
        let format = TextFormat {
            font_family: "monospace".to_owned(),
            font_size: 20.0,
            bold: true,
            italic: true,
            alignment: TextAlignment::Center,
        };

        let _flags = apply_text_format(&mut engine, &config, &format);

        let style = config.read().pens_config.typewriter_config.text_style.clone();
        assert_eq!(style.font_family, "monospace");
        assert_eq!(style.font_size, 20.0);
        assert_eq!(style.font_weight, BOLD_WEIGHT);
        assert!(matches!(style.font_style, FontStyle::Italic));
        assert!(matches!(style.alignment, TextAlignment::Center));
    }

    #[test]
    fn shape_style_sets_style_and_fill() {
        let config = EngineConfigShared::default();
        let fill = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

        apply_shape_style(&config, ShapeStyle { rough: true, fill: Some(fill) });

        let shaper = config.read().pens_config.shaper_config.clone();
        assert!(matches!(shaper.style, ShaperStyle::Rough));
        assert_eq!(shaper.smooth_options.fill_color, Some(fill));
        assert_eq!(shaper.rough_options.fill_color, Some(fill));
    }
}
