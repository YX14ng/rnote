//! A document held by the engine: rendering, pen input, history and saving.

use anyhow::Context;
use futures::FutureExt;
use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_compose::Color;
use rnote_engine::engine::{EngineConfigShared, EngineSnapshot, EngineTaskReceiver};
use rnote_engine::pens::PenStyle;
use rnote_engine::pens::pensconfig::selectorconfig::SelectorStyle;
use rnote_engine::{Engine, WidgetFlags};

use crate::frame_cache::FrameCache;
use crate::image_layer::ImageLayer;
use crate::pdf_export::PdfExportJob;
use crate::pen_input::PenClock;
use crate::render::{self, BackgroundTiles, PixelLayout, Viewport};

pub use clipboard::ClipboardEntry;
#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub(crate) use ink::pending_json;
pub use text_selection::TextEmphasis;

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
    /// The brush stroke being drawn was just replaced by the shape it looks like; the rest
    /// of that touch is ignored.
    pub shape_snapped: bool,
    /// The selector is drawing a lasso, which the app draws itself; frames leave it out.
    pub lasso: bool,
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
    // PDF pages and pictures, rasterized once and reused by later frames.
    images: ImageLayer,
    // The last frame, so the next one only redraws where something changed.
    frames: FrameCache,
    // The pen style used for `PenMode::Pen`, kept in sync with the engine's.
    pen_style: PenStyle,
    // Whether the current pen takes keyboard input, as last reported by the engine.
    text_input: bool,
    modified: bool,
    // How many changes the document has had, so a save marks saved only what it took.
    edits: u64,
    recognition: recognition::Recognition,
    // When the samples of the stroke being drawn were taken, for the engine's smoothing.
    pen_clock: PenClock,
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
        // Frames are drawn from the strokes' geometry (see `render`): the rasters the engine
        // makes for the GTK frontend would only cost time while writing, undoing and opening.
        engine.set_stroke_rendering_enabled(false);
        let config = EngineConfigShared::default();
        {
            let mut config = config.write();
            config.pens_config.pen_mode_pen_style = PenStyle::Brush;
            config.pens_config.pen_mode_eraser_style = PenStyle::Eraser;
            // Select by enclosing with a lasso, like other note-taking apps; the engine's
            // default is a rectangle.
            config.pens_config.selector_config.style = SelectorStyle::Polygon;
        }
        // Installing the config reinstalls the pens for those styles.
        let flags = engine.install_config(&config, /* data_dir */ None);
        let tasks = engine.take_engine_tasks_rx();
        let mut document = Self {
            engine,
            config,
            tasks,
            tiles: BackgroundTiles::default(),
            images: ImageLayer::default(),
            frames: FrameCache::default(),
            pen_style: PenStyle::Brush,
            text_input: false,
            modified: false,
            edits: 0,
            recognition: recognition::Recognition::default(),
            pen_clock: PenClock::default(),
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

    /// The top-left corner of what the view showed when the document was saved, in document
    /// coordinates: where it was last worked on.
    #[must_use]
    pub fn saved_view_origin(&self) -> Vector2 {
        self.engine.camera.viewport().mins
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
            shape_snapped: false,
            lasso: self.engine.penholder.selector_lassoing(),
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
        render::render_rgba(
            &self.engine,
            &self.tiles,
            &self.images,
            Some(&self.frames),
            self.bounds(),
            viewport,
            layout,
            out,
        )
    }

    /// Like [`Self::render_rgba`], always drawn whole and without touching the kept frame:
    /// for other views of the document, e.g. the thumbnails of its pages.
    pub fn render_rgba_whole(
        &mut self,
        viewport: Viewport,
        layout: PixelLayout,
        out: &mut [u8],
    ) -> anyhow::Result<()> {
        render::render_rgba(
            &self.engine,
            &self.tiles,
            &self.images,
            None,
            self.bounds(),
            viewport,
            layout,
            out,
        )
    }

    /// Marks the document as having unsaved changes, e.g. after restoring it from a
    /// recovery copy.
    pub fn mark_modified(&mut self) {
        self.touch();
    }

    /// Records a change: the document has unsaved changes.
    fn touch(&mut self) {
        self.modified = true;
        self.edits = self.edits.wrapping_add(1);
    }

    /// Serializes the document as an `.rnote` file. `file_name` is stored in its metadata.
    ///
    /// The document stays modified until [`Self::mark_saved`] is called, which the caller
    /// does once the bytes are actually written.
    pub fn save_to_bytes(&self, file_name: &str) -> anyhow::Result<Vec<u8>> {
        self.begin_save(file_name).wait()
    }

    /// The document as a PDF named `title`, as it looks: background, pattern and every
    /// stroke, a page per sheet (a document without sheets is cut into pages of its format).
    #[cfg(test)]
    pub fn export_pdf(&self, title: &str) -> anyhow::Result<Vec<u8>> {
        self.pdf_export_job(title).finish()
    }

    /// The export of the document as a PDF, to draw a sheet at a time; see [`PdfExportJob`].
    pub fn pdf_export_job(&self, title: &str) -> PdfExportJob {
        PdfExportJob::new(&self.engine, title)
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
        // Unclamped: the view may center a document smaller than the screen, and the camera
        // has to match it exactly, or the eraser and the document's growth go by another area.
        let offset = Vector2::new(viewport.x, viewport.y) * camera.total_zoom();
        flags |= camera.set_offset_unclamped(offset);
        flags
    }

    fn finish_edit(&mut self, flags: WidgetFlags, bounds_before: Aabb) -> EditState {
        self.finish_operation(flags);
        EditState { bounds_changed: self.bounds() != bounds_before, ..self.edit_state() }
    }

    fn finish_operation(&mut self, flags: WidgetFlags) {
        if flags.store_modified {
            self.touch();
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

mod clipboard;
mod editing;
mod ink;
mod pages;
mod recognition;
mod saving;
#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub(crate) use saving::PendingSave;
mod selection;
mod settings;
#[cfg(test)]
mod tests;
mod text_selection;
