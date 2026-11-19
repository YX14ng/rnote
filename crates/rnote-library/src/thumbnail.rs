//! Thumbnails of notebooks, rendered with the engine.

use std::io::Cursor;

use anyhow::Context;
use futures::executor::block_on;
use rnote_compose::SplitOrder;
use rnote_engine::Engine;
use rnote_engine::engine::EngineSnapshot;

use crate::Notebook;

/// A PNG of the notebook's first page with content (the first page of an empty one), at
/// most `size_px` wide and high.
///
/// The page is drawn straight onto pixels at the thumbnail's size: going through an SVG of
/// it, as before, encoded PDF pages as PNG and decoded them again, for every thumbnail.
pub fn render_thumbnail(rnote_bytes: Vec<u8>, size_px: u32) -> anyhow::Result<Vec<u8>> {
    let snapshot = block_on(EngineSnapshot::load_from_rnote_bytes(rnote_bytes))
        .context("Loading the notebook failed")?;
    let mut engine = Engine::default();
    // Drawn once from the strokes themselves: rasters for an editor would be wasted work.
    engine.set_stroke_rendering_enabled(false);
    let _ = engine.load_snapshot(snapshot);
    let bounds = engine
        .pages_bounds_w_content(SplitOrder::default())
        .into_iter()
        .next()
        .context("The notebook has no pages")?;
    let page = engine.extract_content_in_bounds(bounds);
    let extents = bounds.extents();
    let scale = f64::from(size_px) / extents[0].max(extents[1]);
    let width = pixels(extents[0] * scale);
    let height = pixels(extents[1] * scale);
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
        .context("Creating the thumbnail surface failed")?;
    {
        let cx = cairo::Context::new(&surface).context("Creating the thumbnail context failed")?;
        cx.scale(scale, scale);
        cx.translate(-bounds.mins[0], -bounds.mins[1]);
        page.draw_to_cairo(&cx, true, true, false, 0.0, scale)?;
    }
    surface.flush();
    let stride = usize::try_from(surface.stride()).context("Negative stride")?;
    let data = surface.data().context("Reading the thumbnail pixels failed")?;
    let (w, h) = (width as u32, height as u32);
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for row in data.chunks(stride).take(h as usize) {
        for &[b, g, r, a] in row[..w as usize * 4].as_chunks::<4>().0 {
            // Cairo keeps premultiplied BGRA in native order; PNG wants straight RGBA.
            let straight = |c: u8| {
                if a == 0 {
                    0
                } else {
                    ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
                }
            };
            rgba.extend_from_slice(&[straight(r), straight(g), straight(b), a]);
        }
    }
    let image =
        image::RgbaImage::from_raw(w, h, rgba).context("Building the thumbnail image failed")?;
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

fn pixels(length: f64) -> i32 {
    length.round().clamp(1.0, f64::from(i32::MAX)) as i32
}

/// The thumbnail as it used to be made, through an SVG of the page: what the direct drawing
/// is checked against.
#[cfg(test)]
pub(crate) fn render_thumbnail_through_svg(
    rnote_bytes: Vec<u8>,
    size_px: u32,
) -> anyhow::Result<Vec<u8>> {
    let snapshot = block_on(EngineSnapshot::load_from_rnote_bytes(rnote_bytes))
        .context("Loading the notebook failed")?;
    let mut engine = Engine::default();
    let _ = engine.load_snapshot(snapshot);
    let page = engine
        .extract_pages_content(SplitOrder::default())
        .into_iter()
        .next()
        .context("The notebook has no pages")?;
    let svg = page.gen_svg(true, true, false, 0.0)?.context("The page is empty")?;
    let extents = svg.bounds.extents();
    let scale = f64::from(size_px) / extents[0].max(extents[1]);
    let image = svg.gen_image(scale)?.into_imgbuf()?;
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

/// The cache key of `notebook`'s thumbnail, `<path>-<version>`: the same while its path,
/// modification time and size don't change, and the same whatever Rust built it, so a
/// new compiler doesn't make every thumbnail be drawn again.
#[must_use]
pub fn thumbnail_key(notebook: &Notebook) -> String {
    let mut version = Vec::with_capacity(16);
    version.extend_from_slice(&notebook.modified_ms.to_le_bytes());
    version.extend_from_slice(&notebook.size.to_le_bytes());
    format!("{}-{:016x}", path_key(&notebook.path), fnv1a(&version))
}

/// The first part of [`thumbnail_key`]: the same for every version of a notebook.
pub(crate) fn path_key(path: &str) -> String {
    format!("{:016x}", fnv1a(path.as_bytes()))
}

/// The 64-bit FNV-1a hash of `bytes`.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
