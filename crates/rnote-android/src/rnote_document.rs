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
use rnote_engine::engine::{EngineConfigShared, EngineSnapshot, EngineTaskReceiver};
use rnote_engine::pens::pensconfig::BrushConfig;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::pens::pensconfig::eraserconfig::EraserStyle;
use rnote_engine::pens::{PenMode, PenStyle};
use rnote_engine::{Engine, WidgetFlags};

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

    /// Sets the color and width of new brush strokes and shapes, for every style.
    pub fn set_brush(&mut self, color: Color, width: f64) {
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
mod tests {
    use super::*;
    use crate::render::{BYTES_PER_PIXEL, DESK_COLOR};
    use rnote_engine::document::Layout;
    use rnote_engine::document::background::PatternStyle;

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
        document.engine.document.config.background.color =
            Color { r: red, g: green, b: blue, a: 1.0 };
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
        let samples: Vec<PenSample> = (0..=20)
            .map(|i| PenSample { x: 100.0 + 10.0 * f64::from(i), y, pressure: 1.0 })
            .collect();
        let (moves, last) = samples.split_at(samples.len() - 1);
        document.handle_pen(PenPhase::Down, Tool::Brush, moves, IDENTITY, VIEW);
        document.handle_pen(PenPhase::Up, Tool::Brush, last, IDENTITY, VIEW)
    }

    /// Counts the pixels of a `size` x `size` render at `zoom` that are darker than mid gray.
    fn count_dark_pixels(
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
        let state = document.handle_pen(
            PenPhase::Up,
            Tool::Shaper,
            &[corner(300.0, 300.0)],
            IDENTITY,
            VIEW,
        );

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
}
