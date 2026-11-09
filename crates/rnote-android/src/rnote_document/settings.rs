//! Brush, shape, text and document settings.

use parry2d_f64::bounding_volume::Aabb;
use rnote_compose::Color;
use rnote_compose::builders::ShapeBuilderType;
use rnote_engine::WidgetFlags;
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::pens::pensconfig::BrushConfig;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::pens::pensconfig::eraserconfig::EraserStyle;

use super::{EditState, RnoteDocument};
use crate::document_settings::{self, DocumentSettings, ShapeStyle, TextFormat};

impl RnoteDocument {
    /// Sets the color and width of new brush strokes and shapes, for every style.
    pub fn set_brush(&mut self, color: Color, width: f64) {
        // Recolors the text being edited, like the desktop app; new text takes it from the
        // config below.
        let flags = document_settings::apply_text_color(&mut self.engine, color);
        self.finish_operation(flags);
        let width = width.clamp(BrushConfig::STROKE_WIDTH_MIN, BrushConfig::STROKE_WIDTH_MAX);
        let mut config = self.config.write();
        config.pens_config.set_all_stroke_colors(color);
        let brush = &mut config.pens_config.brush_config;
        brush.marker_options.stroke_width = width;
        brush.solid_options.stroke_width = width;
        brush.textured_options.stroke_width = width;
        let shaper = &mut config.pens_config.shaper_config;
        shaper.smooth_options.stroke_width = width;
        shaper.rough_options.stroke_width = width;
    }

    /// Sets the shape that the shaper draws.
    pub fn set_shape(&mut self, shape: ShapeBuilderType) {
        self.config.write().pens_config.shaper_config.builder_type = shape;
    }

    /// Sets the look of brush strokes.
    pub fn set_brush_style(&mut self, style: BrushStyle) {
        self.config.write().pens_config.brush_config.style = style;
    }

    /// Sets whether the eraser removes whole strokes or only the parts it touches.
    pub fn set_eraser_style(&mut self, style: EraserStyle) {
        self.config.write().pens_config.eraser_config.style = style;
    }

    /// Sets the format of new text, and of the text being edited if any.
    pub fn set_text_format(&mut self, format: &TextFormat) -> EditState {
        let bounds_before = self.bounds();
        let flags = document_settings::apply_text_format(&mut self.engine, &self.config, format);
        self.finish_edit(flags, bounds_before)
    }

    /// Sets the style and fill of new shapes.
    pub fn set_shape_style(&mut self, style: ShapeStyle) {
        document_settings::apply_shape_style(&self.config, style);
    }

    /// The page size, layout and background.
    #[must_use]
    pub fn document_settings(&self) -> DocumentSettings {
        document_settings::document_settings(&self.engine)
    }

    /// Changes the page size, in millimeters.
    pub fn set_page_size(&mut self, width_mm: f64, height_mm: f64) -> EditState {
        let bounds_before = self.bounds();
        let flags = document_settings::apply_page_size(&mut self.engine, width_mm, height_mm);
        self.finish_settings_change(flags, bounds_before)
    }

    /// Changes how the document grows.
    pub fn set_layout(&mut self, layout: Layout) -> EditState {
        let bounds_before = self.bounds();
        let flags = document_settings::apply_layout(&mut self.engine, layout);
        self.finish_settings_change(flags, bounds_before)
    }

    /// Changes the background color and pattern.
    pub fn set_background(
        &mut self,
        color: Color,
        pattern: PatternStyle,
        pattern_color: Color,
    ) -> EditState {
        let bounds_before = self.bounds();
        document_settings::apply_background(&mut self.engine, color, pattern, pattern_color);
        self.finish_settings_change(WidgetFlags::default(), bounds_before)
    }

    /// Shows or hides the page outlines.
    pub fn set_page_borders(&mut self, show: bool) -> EditState {
        let bounds_before = self.bounds();
        document_settings::apply_page_borders(&mut self.engine, show);
        self.finish_settings_change(WidgetFlags::default(), bounds_before)
    }

    /// Document settings are saved with the document but don't go through the history.
    fn finish_settings_change(&mut self, flags: WidgetFlags, bounds_before: Aabb) -> EditState {
        self.modified = true;
        self.finish_edit(flags, bounds_before)
    }
}
