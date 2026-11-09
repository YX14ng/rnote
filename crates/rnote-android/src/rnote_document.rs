//! A document held by the engine: rendering, pen input, history and saving.

use std::time::Instant;

use anyhow::Context;
use futures::FutureExt;
use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_compose::Color;
use rnote_engine::engine::{EngineConfigShared, EngineSnapshot, EngineTaskReceiver};
use rnote_engine::pens::pensconfig::BrushConfig;
use rnote_engine::pens::{PenMode, PenStyle};
use rnote_engine::{Engine, WidgetFlags};

use crate::pen_input::{self, PenPhase, PenSample, Tool};
use crate::render::{self, PixelLayout, Viewport};

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
        let mut flags = engine.install_config(&config, /* data_dir */ None);
        flags |= engine.change_pen_style(PenStyle::Brush);
        let tasks = engine.take_engine_tasks_rx();
        let mut document = Self { engine, config, tasks, modified: false };
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
        }
    }

    /// Renders the region of the document given by `viewport` into `out`.
    ///
    /// See [`render::render_rgba`] for the pixel format.
    pub fn render_rgba(
        &self,
        viewport: Viewport,
        layout: PixelLayout,
        out: &mut [u8],
    ) -> anyhow::Result<()> {
        render::render_rgba(&self.engine, self.bounds(), viewport, layout, out)
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
            Tool::Brush => PenMode::Pen,
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

    /// Sets the color and width of new brush strokes, for every brush style.
    pub fn set_brush(&mut self, color: Color, width: f64) {
        let width = width.clamp(BrushConfig::STROKE_WIDTH_MIN, BrushConfig::STROKE_WIDTH_MAX);
        let mut config = self.config.write();
        let brush = &mut config.pens_config.brush_config;
        brush.marker_options.stroke_color = Some(color);
        brush.marker_options.stroke_width = width;
        brush.solid_options.stroke_color = Some(color);
        brush.solid_options.stroke_width = width;
        brush.textured_options.stroke_color = Some(color);
        brush.textured_options.stroke_width = width;
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

    fn render_pixel(document: &RnoteDocument, x: f64, y: f64) -> [u8; 4] {
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
    fn count_dark_pixels(document: &RnoteDocument, x: f64, y: f64, zoom: f64, size: u32) -> usize {
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
        let document = RnoteDocument::try_open(empty_document_bytes(1.0, 0.5, 0.0))
            .expect("an empty document should load");
        let center = document.bounds().center();

        let pixel = render_pixel(&document, center.x, center.y);

        assert_pixel_near(pixel, [to_byte(1.0), to_byte(0.5), to_byte(0.0), 255]);
    }

    #[test]
    fn renders_desk_outside_document() {
        let document = RnoteDocument::try_open(empty_document_bytes(1.0, 1.0, 1.0))
            .expect("an empty document should load");
        let mins = document.bounds().mins;

        let pixel = render_pixel(&document, mins.x - 100.0, mins.y - 100.0);

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
        let document = plain_document();
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
        let near_origin = count_dark_pixels(&document, 100.0, 100.0, 40.0, 400);
        let far_from_origin = count_dark_pixels(&document, 100.0, 1000.0, 40.0, 400);

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
        assert_pixel_near(render_pixel(&document, 200.0, 200.0), [255, 0, 0, 255]);
    }

    #[test]
    fn undo_and_redo_remove_and_restore_the_stroke() {
        let mut document = plain_document();
        document.set_brush(RED, 6.0);
        draw_line(&mut document, 200.0);

        let undone = document.undo();
        assert!(undone.can_redo, "got {undone:?}");
        assert_pixel_near(render_pixel(&document, 200.0, 200.0), [255, 255, 255, 255]);

        document.redo();
        assert_pixel_near(render_pixel(&document, 200.0, 200.0), [255, 0, 0, 255]);
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

        assert_pixel_near(render_pixel(&document, 200.0, 200.0), [255, 255, 255, 255]);
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
        let reopened = RnoteDocument::try_open(bytes).expect("the saved file should load");
        assert_pixel_near(render_pixel(&reopened, 200.0, 200.0), [255, 0, 0, 255]);
    }
}
