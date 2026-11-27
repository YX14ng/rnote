//! Exporting a notebook as a PDF one sheet at a time, so the app can tell how far it got
//! and stop when asked: the engine's own export makes the whole PDF in one go.

use std::collections::VecDeque;

use anyhow::Context as _;
use rnote_engine::Engine;
use rnote_engine::engine::StrokeContent;
use rnote_engine::engine::export::DocExportPrefs;

/// PDF surfaces measure in points, 72 to the inch.
const PT_IN_INCH: f64 = 72.0;
/// The margin around each sheet: `DocExportPrefs::MARGIN`, which the engine keeps private.
const MARGIN: f64 = 0.0;

/// A PDF being made, a page per sheet. Taken from the document on its thread (quick), then
/// drawn on another one, step by step; dropping it midway cancels the export.
pub struct PdfExportJob {
    title: String,
    page_size: (f64, f64),
    scale: f64,
    pages: VecDeque<StrokeContent>,
    total: usize,
    done: usize,
    output: Option<(cairo::PdfSurface, cairo::Context)>,
}

// SAFETY: The cairo surface and context are only created, used and dropped by whichever
// thread holds the job, one call at a time; cairo objects aren't tied to a thread.
unsafe impl Send for PdfExportJob {}

impl PdfExportJob {
    /// The export of `engine`'s document titled `title`: every sheet, with its background
    /// and pattern, as the column of pages shows them.
    pub fn new(engine: &Engine, title: &str) -> Self {
        let order = DocExportPrefs::default().page_order;
        let pages: VecDeque<StrokeContent> = engine.extract_all_pages_content(order).into();
        let format = &engine.document.config.format;
        let scale = PT_IN_INCH / format.dpi();
        let size = format.size();
        Self {
            title: title.to_owned(),
            page_size: (size[0] * scale, size[1] * scale),
            scale,
            total: pages.len(),
            pages,
            done: 0,
            output: None,
        }
    }

    /// How many sheets the PDF has.
    pub fn total(&self) -> usize {
        self.total
    }

    /// How many sheets are drawn.
    pub fn done(&self) -> usize {
        self.done
    }

    /// Draws the next sheet onto its own page.
    pub fn draw_next(&mut self) -> anyhow::Result<()> {
        let Some(page) = self.pages.pop_front() else {
            return Ok(());
        };
        let (_, cx) = self.output()?;
        // A sheet without bounds has nothing to draw, as in the engine's export.
        if let Some(bounds) = page.bounds() {
            cx.save()?;
            cx.translate(-bounds.mins[0], -bounds.mins[1]);
            page.draw_to_cairo(cx, true, true, false, MARGIN, Engine::STROKE_EXPORT_IMAGE_SCALE)?;
            cx.show_page().context("Showing a page of the PDF failed")?;
            cx.restore()?;
        }
        self.done += 1;
        Ok(())
    }

    /// The PDF, once every sheet is drawn.
    pub fn finish(mut self) -> anyhow::Result<Vec<u8>> {
        while self.done < self.total {
            self.draw_next()?;
        }
        self.output()?;
        let (surface, cx) = self.output.take().context("The PDF has no output")?;
        drop(cx);
        let stream = surface
            .finish_output_stream()
            .map_err(|e| anyhow::anyhow!("Finishing the PDF failed: {e:?}"))?;
        let bytes = stream
            .downcast::<Vec<u8>>()
            .map_err(|_| anyhow::anyhow!("The PDF stream isn't bytes"))?;
        Ok(*bytes)
    }

    /// The PDF surface and its context, made on first use by the thread drawing.
    fn output(&mut self) -> anyhow::Result<&(cairo::PdfSurface, cairo::Context)> {
        if self.output.is_none() {
            let surface =
                cairo::PdfSurface::for_stream(self.page_size.0, self.page_size.1, Vec::<u8>::new())
                    .context("Creating the PDF surface failed")?;
            surface
                .set_metadata(cairo::PdfMetadata::Title, &self.title)
                .context("Setting the PDF title failed")?;
            let cx = cairo::Context::new(&surface).context("Creating the PDF context failed")?;
            cx.scale(self.scale, self.scale);
            self.output = Some((surface, cx));
        }
        self.output.as_ref().context("The PDF has no output")
    }
}
