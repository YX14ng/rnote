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

/// How the PDF comes out.
#[derive(Debug, Clone, Default)]
pub struct PdfExportOptions {
    /// White paper and dark ink, e.g. to print a notebook written in white on dark paper:
    /// the background turns white, a dark paper's lines are inverted and light ink darkens.
    pub on_white_paper: bool,
    /// Only the sheets from the first to the last of these (0-based), not every sheet.
    pub sheets: Option<(usize, usize)>,
}

/// Draws `page` onto `cx` as a sheet of the PDF, `on_white_paper` or as it is.
pub fn draw_sheet(
    page: &StrokeContent,
    cx: &cairo::Context,
    on_white_paper: bool,
) -> anyhow::Result<()> {
    page.draw_to_cairo(cx, true, true, on_white_paper, MARGIN, Engine::STROKE_EXPORT_IMAGE_SCALE)
}

/// A PDF being made, a page per sheet. Taken from the document on its thread (quick), then
/// drawn on another one, step by step; dropping it midway cancels the export.
pub struct PdfExportJob {
    title: String,
    on_white_paper: bool,
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
    /// The export of `engine`'s document titled `title`: its sheets (all of them, or those
    /// `options` asks for), with their background and pattern, as the column of pages shows
    /// them.
    pub fn new(engine: &Engine, title: &str, options: &PdfExportOptions) -> Self {
        let order = DocExportPrefs::default().page_order;
        let mut all = engine.extract_all_pages_content(order);
        if let Some((first, last)) = options.sheets {
            let last = last.min(all.len().saturating_sub(1));
            let first = first.min(last);
            all.truncate(last + 1);
            all.drain(..first);
        }
        let pages: VecDeque<StrokeContent> = all.into();
        let format = &engine.document.config.format;
        let scale = PT_IN_INCH / format.dpi();
        let size = format.size();
        Self {
            title: title.to_owned(),
            on_white_paper: options.on_white_paper,
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
        let on_white_paper = self.on_white_paper;
        let (_, cx) = self.output()?;
        // A sheet without bounds has nothing to draw, as in the engine's export.
        if let Some(bounds) = page.bounds() {
            cx.save()?;
            cx.translate(-bounds.mins[0], -bounds.mins[1]);
            draw_sheet(&page, cx, on_white_paper)?;
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
