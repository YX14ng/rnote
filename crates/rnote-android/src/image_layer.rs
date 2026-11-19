//! PDF pages and pictures, which lie under the ink, drawn from rasters kept between frames.
//!
//! Drawing them from their source every frame is the slowest part of a frame: a vector PDF
//! page is parsed and rendered by librsvg again, and a bitmap page is converted pixel by pixel
//! again. Here each one is rasterized once at a scale a little above the zoom shown, and later
//! frames only paint that raster, scaled. A page too big to keep at that zoom is drawn from
//! its source, as before.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use anyhow::Context;
use parry2d_f64::bounding_volume::Aabb;
use rnote_compose::shapes::Shapeable;
use rnote_engine::Drawable;
use rnote_engine::strokes::{BitmapImage, Stroke};

/// Pixels kept in rasters at most, all together (about 80 MB).
const BUDGET_PIXELS: u64 = 20_000_000;

/// Rasters per doubling of the zoom: a raster serves zooms up to 41 % below its own.
const STEPS_PER_OCTAVE: f64 = 2.0;

/// The rasters of the images drawn lately, least recently used first.
pub(crate) struct ImageLayer {
    budget: u64,
    rasters: RefCell<Vec<Raster>>,
    rasterized: Cell<usize>,
}

struct Raster {
    key: Key,
    pixels: u64,
    surface: cairo::ImageSurface,
}

/// The image (the same allocation, at the same place) and the scale it was rasterized at.
#[derive(PartialEq)]
struct Key {
    stroke: usize,
    bounds: [u64; 4],
    scale: u64,
}

impl Default for ImageLayer {
    fn default() -> Self {
        Self::with_budget(BUDGET_PIXELS)
    }
}

impl ImageLayer {
    /// A layer keeping up to `budget` pixels; 0 draws every image from its source.
    pub(crate) fn with_budget(budget: u64) -> Self {
        Self { budget, rasters: RefCell::new(Vec::new()), rasterized: Cell::new(0) }
    }

    /// Whether `stroke` is drawn by this layer: a PDF page or a picture.
    pub(crate) fn takes(stroke: &Stroke) -> bool {
        matches!(stroke, Stroke::VectorImage(_) | Stroke::BitmapImage(_))
    }

    /// How many rasters were made so far.
    #[cfg(test)]
    pub(crate) fn rasterizations(&self) -> usize {
        self.rasterized.get()
    }

    /// Draws `stroke` to `cx`, whose user space is the document's and which shows it at
    /// `zoom` target pixels per document unit.
    pub(crate) fn draw(
        &self,
        cx: &cairo::Context,
        stroke: &Arc<Stroke>,
        zoom: f64,
    ) -> anyhow::Result<()> {
        let bounds = stroke.bounds();
        let scale = match stroke.as_ref() {
            // More pixels than the picture has add nothing.
            Stroke::BitmapImage(bitmap) => bucket(zoom).min(native_scale(bitmap, bounds)),
            _ => bucket(zoom),
        };
        let (width, height) =
            (pixels(bounds.extents().x * scale), pixels(bounds.extents().y * scale));
        let size = u64::from(width) * u64::from(height);
        if size == 0 || size > self.budget {
            return stroke.draw_to_cairo(cx, zoom);
        }
        let key = Key {
            stroke: Arc::as_ptr(stroke) as usize,
            bounds: [bounds.mins.x, bounds.mins.y, bounds.maxs.x, bounds.maxs.y].map(f64::to_bits),
            scale: scale.to_bits(),
        };
        let surface = match self.take(&key) {
            Some(surface) => surface,
            None => {
                self.rasterized.set(self.rasterized.get() + 1);
                rasterize(stroke, bounds, scale, width, height)?
            }
        };
        paint(cx, &surface, bounds, scale)?;
        self.keep(Raster { key, pixels: size, surface });
        Ok(())
    }

    /// The raster for `key`, removed from the list so it can go back as the most recent.
    fn take(&self, key: &Key) -> Option<cairo::ImageSurface> {
        let mut rasters = self.rasters.borrow_mut();
        let at = rasters.iter().position(|raster| raster.key == *key)?;
        Some(rasters.remove(at).surface)
    }

    fn keep(&self, raster: Raster) {
        let mut rasters = self.rasters.borrow_mut();
        rasters.push(raster);
        let mut total: u64 = rasters.iter().map(|raster| raster.pixels).sum();
        while total > self.budget && rasters.len() > 1 {
            total -= rasters.remove(0).pixels;
        }
    }
}

/// The scale to rasterize at for `zoom`: the next step up.
fn bucket(zoom: f64) -> f64 {
    let steps = (zoom.max(f64::MIN_POSITIVE).log2() * STEPS_PER_OCTAVE).ceil();
    (steps / STEPS_PER_OCTAVE).exp2()
}

/// The scale at which `bitmap`, `bounds` large, shows each of its pixels once.
fn native_scale(bitmap: &BitmapImage, bounds: Aabb) -> f64 {
    let width = bounds.extents().x;
    if width > 0.0 { f64::from(bitmap.image.pixel_width) / width } else { 1.0 }
}

fn pixels(length: f64) -> u32 {
    if length.is_finite() && length > 0.0 {
        length.ceil().min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

fn rasterize(
    stroke: &Stroke,
    bounds: Aabb,
    scale: f64,
    width: u32,
    height: u32,
) -> anyhow::Result<cairo::ImageSurface> {
    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        i32::try_from(width).context("Image raster too wide")?,
        i32::try_from(height).context("Image raster too tall")?,
    )
    .context("Creating an image raster failed")?;
    let cx = cairo::Context::new(&surface).context("Creating the raster's context failed")?;
    cx.scale(scale, scale);
    cx.translate(-bounds.mins.x, -bounds.mins.y);
    stroke.draw_to_cairo(&cx, scale)?;
    drop(cx);
    surface.flush();
    Ok(surface)
}

fn paint(
    cx: &cairo::Context,
    surface: &cairo::ImageSurface,
    bounds: Aabb,
    scale: f64,
) -> anyhow::Result<()> {
    cx.save()?;
    cx.rectangle(bounds.mins.x, bounds.mins.y, bounds.extents().x, bounds.extents().y);
    cx.clip();
    cx.translate(bounds.mins.x, bounds.mins.y);
    cx.scale(1.0 / scale, 1.0 / scale);
    cx.set_source_surface(surface, 0.0, 0.0)?;
    cx.source().set_filter(cairo::Filter::Good);
    cx.paint()?;
    cx.restore()?;
    Ok(())
}
