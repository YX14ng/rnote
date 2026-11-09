//! A document held by the engine: rendering, pen input, history and saving.

use std::time::Instant;

use anyhow::Context;
use futures::FutureExt;
use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_compose::builders::ShapeBuilderType;
use std::collections::HashSet;

use rnote_compose::penevent::{KeyboardKey, ModifierKey};
use rnote_compose::{Color, PenEvent};
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::engine::{EngineConfigShared, EngineSnapshot, EngineTaskReceiver};
use rnote_engine::pens::pensconfig::BrushConfig;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::pens::pensconfig::eraserconfig::EraserStyle;
use rnote_engine::pens::{PenMode, PenStyle};
use rnote_engine::{Engine, WidgetFlags};

use crate::document_settings::{self, DocumentSettings, ShapeStyle, TextFormat};
use crate::pen_input::{self, PenPhase, PenSample, Tool};
use crate::render::{self, BackgroundTiles, PixelLayout, Viewport};

/// The editing state the UI shows after an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditState {
    /// The document has changes that were not saved.
    pub modified: bool,
    /// There is something to undo.
    pub can_undo: bool,
    /// There is something to redo.
    pub can_redo: bool,
    /// The document bounds changed, e.g. because it grew to fit new strokes.
    pub bounds_changed: bool,
    /// Strokes are selected, so selection actions apply.
    pub has_selection: bool,
    /// The text tool is editing text, so the keyboard should be shown.
    pub text_input: bool,
}

/// Size of the view showing the document, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// A document loaded into the engine.
pub struct RnoteDocument {
    engine: Engine,
    config: EngineConfigShared,
    // Rendering tasks the engine queues for its own (GTK) image cache.
    tasks: Option<EngineTaskReceiver>,
    tiles: BackgroundTiles,
    // The pen style used for `PenMode::Pen`, kept in sync with the engine's.
    pen_style: PenStyle,
    // Whether the current pen takes keyboard input, as last reported by the engine.
    text_input: bool,
    modified: bool,
}

impl RnoteDocument {
    /// Creates an empty document with the default format and background.
    #[must_use]
    pub fn new_empty() -> Self {
        Self::with_engine(Engine::default())
    }

    /// Loads a document from the contents of an `.rnote` file.
    pub fn try_open(rnote_bytes: Vec<u8>) -> anyhow::Result<Self> {
        let snapshot =
            futures::executor::block_on(EngineSnapshot::load_from_rnote_bytes(rnote_bytes))
                .context("Loading the .rnote file failed")?;
        let mut document = Self::with_engine(Engine::default());
        let flags = document.engine.load_snapshot(snapshot);
        document.finish_operation(flags);
        Ok(document)
    }

    fn with_engine(mut engine: Engine) -> Self {
        let config = EngineConfigShared::default();
        {
            let mut config = config.write();
            config.pens_config.pen_mode_pen_style = PenStyle::Brush;
            config.pens_config.pen_mode_eraser_style = PenStyle::Eraser;
        }
        // Installing the config reinstalls the pens for those styles.
        let flags = engine.install_config(&config, /* data_dir */ None);
        let tasks = engine.take_engine_tasks_rx();
        let mut document = Self {
            engine,
            config,
            tasks,
            tiles: BackgroundTiles::default(),
            pen_style: PenStyle::Brush,
            text_input: false,
            modified: false,
        };
        document.finish_operation(flags);
        document
    }

    /// Bounds of the whole document in document coordinates, including empty pages.
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        let document = &self.engine.document;
        Aabb::new(
            Vector2::new(document.x, document.y),
            Vector2::new(document.x + document.width, document.y + document.height),
        )
    }

    /// The document background color.
    #[must_use]
    pub fn background_color(&self) -> Color {
        self.engine.document.config.background.color
    }

    /// The current editing state.
    #[must_use]
    pub fn edit_state(&self) -> EditState {
        EditState {
            modified: self.modified,
            can_undo: self.engine.can_undo(),
            can_redo: self.engine.can_redo(),
            bounds_changed: false,
            // Only the selector selects, so skip copying the selection otherwise.
            has_selection: self.pen_style == PenStyle::Selector
                && self.engine.extract_selection_content().is_some(),
            text_input: self.text_input,
        }
    }

    /// Renders the region of the document given by `viewport` into `out`, including what
    /// the current pen shows on top (a shape being drawn, the selection, ...).
    ///
    /// See [`render::render_rgba`] for the pixel format.
    pub fn render_rgba(
        &mut self,
        viewport: Viewport,
        layout: PixelLayout,
        out: &mut [u8],
    ) -> anyhow::Result<()> {
        let flags =
            self.sync_camera(viewport, ViewSize { width: layout.width, height: layout.height });
        self.finish_operation(flags);
        render::render_rgba(&self.engine, &self.tiles, self.bounds(), viewport, layout, out)
    }

    /// Switches the tool used for the pen side of the stylus.
    ///
    /// Leaving the selector finishes the selection, like in the desktop app.
    pub fn set_tool(&mut self, tool: Tool) -> EditState {
        let bounds_before = self.bounds();
        let flags = match tool {
            Tool::Brush => self.use_pen_style(PenStyle::Brush),
            Tool::Shaper => self.use_pen_style(PenStyle::Shaper),
            Tool::Selector => self.use_pen_style(PenStyle::Selector),
            Tool::Typewriter => self.use_pen_style(PenStyle::Typewriter),
            // The eraser is a pen mode, not a style: the pen side keeps its tool.
            Tool::Eraser => WidgetFlags::default(),
        };
        self.finish_edit(flags, bounds_before)
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

    /// Selects every stroke, switching to the selector.
    pub fn select_all(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.ensure_pen_mode() | self.engine.select_all_strokes();
        self.pen_style = PenStyle::Selector;
        self.finish_edit(flags, bounds_before)
    }

    /// Clears the selection.
    pub fn deselect_all(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.ensure_pen_mode() | self.engine.deselect_all_strokes();
        self.pen_style = PenStyle::Selector;
        self.finish_edit(flags, bounds_before)
    }

    /// Deletes the selected strokes.
    pub fn delete_selection(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.trash_selection();
        self.finish_edit(flags, bounds_before)
    }

    /// Duplicates the selected strokes; the copies become the selection.
    pub fn duplicate_selection(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.duplicate_selection();
        self.finish_edit(flags, bounds_before)
    }

    /// Marks the document as having unsaved changes, e.g. after restoring it from a
    /// recovery copy.
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Feeds a batch of stylus samples to the engine.
    ///
    /// `viewport` and `view_size` describe what the user sees, which some pens depend on.
    pub fn handle_pen(
        &mut self,
        phase: PenPhase,
        tool: Tool,
        samples: &[PenSample],
        viewport: Viewport,
        view_size: ViewSize,
    ) -> EditState {
        let bounds_before = self.bounds();
        let mut flags = self.sync_camera(viewport, view_size);
        let pen_mode = match tool {
            Tool::Brush => {
                flags |= self.use_pen_style(PenStyle::Brush);
                PenMode::Pen
            }
            Tool::Shaper => {
                flags |= self.use_pen_style(PenStyle::Shaper);
                PenMode::Pen
            }
            Tool::Selector => {
                flags |= self.use_pen_style(PenStyle::Selector);
                PenMode::Pen
            }
            Tool::Typewriter => {
                flags |= self.use_pen_style(PenStyle::Typewriter);
                PenMode::Pen
            }
            Tool::Eraser => PenMode::Eraser,
        };
        let now = Instant::now();
        for event in pen_input::pen_events(phase, samples) {
            let (_propagation, event_flags) =
                self.engine.handle_pen_event(event, Some(pen_mode), now);
            flags |= event_flags;
        }
        self.finish_edit(flags, bounds_before)
    }

    /// Feeds a key press to the current pen, e.g. editing keys to the text tool, or Enter to
    /// finish a polyline or polygon.
    pub fn handle_key(&mut self, key: KeyboardKey, shift: bool, ctrl: bool) -> EditState {
        let mut modifier_keys = HashSet::new();
        if shift {
            modifier_keys.insert(ModifierKey::KeyboardShift);
        }
        if ctrl {
            modifier_keys.insert(ModifierKey::KeyboardCtrl);
        }
        self.handle_keyboard_event(PenEvent::KeyPressed { keyboard_key: key, modifier_keys })
    }

    /// Inserts committed text at the text tool's cursor.
    pub fn handle_text(&mut self, text: &str) -> EditState {
        self.handle_keyboard_event(PenEvent::Text { text: text.to_owned() })
    }

    fn handle_keyboard_event(&mut self, event: PenEvent) -> EditState {
        let bounds_before = self.bounds();
        // No pen mode: keys go to whatever pen is active.
        let (_propagation, flags) = self.engine.handle_pen_event(event, None, Instant::now());
        self.finish_edit(flags, bounds_before)
    }

    /// Undoes the last change, if any.
    pub fn undo(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = if self.engine.can_undo() {
            self.engine.undo(Instant::now())
        } else {
            WidgetFlags::default()
        };
        self.finish_edit(flags, bounds_before)
    }

    /// Redoes the last undone change, if any.
    pub fn redo(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = if self.engine.can_redo() {
            self.engine.redo(Instant::now())
        } else {
            WidgetFlags::default()
        };
        self.finish_edit(flags, bounds_before)
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

    /// Serializes the document as an `.rnote` file. `file_name` is stored in its metadata.
    ///
    /// The document stays modified until [`Self::mark_saved`] is called, which the caller
    /// does once the bytes are actually written.
    pub fn save_to_bytes(&self, file_name: &str) -> anyhow::Result<Vec<u8>> {
        futures::executor::block_on(self.engine.save_as_rnote_bytes(file_name.to_owned()))
            .context("The save task was cancelled")?
    }

    /// Marks the current state as saved.
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Makes `style` the tool of the pen side, if it isn't already.
    fn use_pen_style(&mut self, style: PenStyle) -> WidgetFlags {
        if self.pen_style == style {
            return WidgetFlags::default();
        }
        self.pen_style = style;
        // Reinstalls the pen and clears the selection, like the desktop app does.
        self.ensure_pen_mode() | self.engine.change_pen_style(style)
    }

    /// Switches the engine to the pen side of the stylus.
    ///
    /// Style changes apply to the current pen mode, which is the eraser after erasing with
    /// the stylus' eraser end.
    fn ensure_pen_mode(&mut self) -> WidgetFlags {
        let (_propagation, flags) =
            self.engine.handle_pen_event(PenEvent::Cancel, Some(PenMode::Pen), Instant::now());
        flags
    }

    fn sync_camera(&mut self, viewport: Viewport, view_size: ViewSize) -> WidgetFlags {
        let camera = &mut self.engine.camera;
        let mut flags =
            camera.set_size(Vector2::new(f64::from(view_size.width), f64::from(view_size.height)));
        flags |= camera.zoom_to(viewport.zoom);
        let offset = Vector2::new(viewport.x, viewport.y) * camera.total_zoom();
        flags |= camera.set_offset(offset, &self.engine.document);
        flags
    }

    /// Document settings are saved with the document but don't go through the history.
    fn finish_settings_change(&mut self, flags: WidgetFlags, bounds_before: Aabb) -> EditState {
        self.modified = true;
        self.finish_edit(flags, bounds_before)
    }

    fn finish_edit(&mut self, flags: WidgetFlags, bounds_before: Aabb) -> EditState {
        self.finish_operation(flags);
        EditState { bounds_changed: self.bounds() != bounds_before, ..self.edit_state() }
    }

    fn finish_operation(&mut self, flags: WidgetFlags) {
        if flags.store_modified {
            self.modified = true;
        }
        if let Some(text_input) = flags.enable_text_input {
            self.text_input = text_input;
        }
        self.drain_tasks();
    }

    /// Lets the engine apply the rendering tasks it queued, so they don't pile up.
    fn drain_tasks(&mut self) {
        let Some(tasks) = self.tasks.as_mut() else {
            return;
        };
        while let Some(Some(task)) = tasks.recv().now_or_never() {
            let (_flags, _quit) = self.engine.handle_engine_task(task);
        }
    }
}

#[cfg(test)]
mod tests;
