//! Rasterization of a document region into RGBA pixel buffers.

use anyhow::{Context, bail};
use parry2d_f64::bounding_volume::{Aabb, BoundingVolume};
use parry2d_f64::math::Vector2;
use rnote_compose::SplitOrder;
use rnote_compose::ext::AabbExt;
use rnote_engine::Engine;
use rnote_engine::engine::StrokeContent;

/// Color of the area around the document, as `[r, g, b]` in `0.0..=1.0`.
pub const DESK_COLOR: [f64; 3] = [0.24, 0.24, 0.26];

/// Bytes per pixel of the RGBA buffers written by [`render_rgba`].
pub const BYTES_PER_PIXEL: usize = 4;

/// The region of the document shown on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Document coordinate shown at the left edge of the target.
    pub x: f64,
    /// Document coordinate shown at the top edge of the target.
    pub y: f64,
    /// Target pixels per document unit.
    pub zoom: f64,
}

/// Size and layout of a pixel buffer that [`render_rgba`] writes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelLayout {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Distance in bytes between the starts of two consecutive rows.
    pub stride: usize,
}

impl PixelLayout {
    /// Minimum buffer length, in bytes, needed to hold the pixels.
    #[must_use]
    pub fn min_len(&self) -> usize {
        if self.height == 0 {
            return 0;
        }
        self.stride * (self.height as usize - 1) + self.width as usize * BYTES_PER_PIXEL
    }
}

/// Renders the region of `engine`'s document given by `viewport` into `out`.
///
/// `out` receives premultiplied RGBA pixels laid out as described by `layout`, which is what
/// Android's `Bitmap.Config.ARGB_8888` uses. Areas outside of `doc_bounds` are filled with
/// [`DESK_COLOR`].
pub(crate) fn render_rgba(
    engine: &Engine,
    doc_bounds: Aabb,
    viewport: Viewport,
    layout: PixelLayout,
    out: &mut [u8],
) -> anyhow::Result<()> {
    if !(viewport.zoom.is_finite() && viewport.zoom > 0.0) {
        bail!("Invalid zoom {}", viewport.zoom);
    }
    if layout.stride < layout.width as usize * BYTES_PER_PIXEL {
        bail!("Stride {} is too small for width {}", layout.stride, layout.width);
    }
    if out.len() < layout.min_len() {
        bail!("Buffer of {} bytes is too small for {layout:?}", out.len());
    }
    if layout.width == 0 || layout.height == 0 {
        return Ok(());
    }

    let mut surface = draw_to_surface(engine, doc_bounds, viewport, layout.width, layout.height)?;
    let src_stride = usize::try_from(surface.stride()).context("Negative cairo stride")?;
    let src = surface.data().context("Accessing the cairo surface data failed")?;
    copy_bgra_to_rgba(&src, src_stride, out, layout);
    Ok(())
}

fn draw_to_surface(
    engine: &Engine,
    doc_bounds: Aabb,
    viewport: Viewport,
    width: u32,
    height: u32,
) -> anyhow::Result<cairo::ImageSurface> {
    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        i32::try_from(width).context("Width too large")?,
        i32::try_from(height).context("Height too large")?,
    )
    .context("Creating the cairo surface failed")?;
    let cx = cairo::Context::new(&surface).context("Creating the cairo context failed")?;

    let [r, g, b] = DESK_COLOR;
    cx.set_source_rgb(r, g, b);
    cx.paint()?;

    let viewport_bounds = Aabb::new(
        Vector2::new(viewport.x, viewport.y),
        Vector2::new(
            viewport.x + f64::from(width) / viewport.zoom,
            viewport.y + f64::from(height) / viewport.zoom,
        ),
    );
    if let Some(visible) = viewport_bounds.intersection(&doc_bounds) {
        cx.scale(viewport.zoom, viewport.zoom);
        cx.translate(-viewport.x, -viewport.y);
        // Same order as the desktop app: background, page borders, then the strokes.
        StrokeContent::default()
            .with_background(engine.document.config.background)
            .with_bounds(visible)
            .draw_to_cairo(
                &cx,
                /* draw_background */ true,
                /* draw_pattern */ true,
                /* optimize_printing */ false,
                /* margin */ 0.0,
                /* image_scale */ viewport.zoom,
            )?;
        draw_page_borders(engine, &cx, doc_bounds, visible, viewport.zoom)?;
        engine.extract_content_in_bounds(visible).draw_to_cairo(
            &cx,
            /* draw_background */ false,
            /* draw_pattern */ false,
            /* optimize_printing */ false,
            /* margin */ 0.0,
            /* image_scale */ viewport.zoom,
        )?;
    }

    drop(cx);
    surface.flush();
    Ok(surface)
}

/// Draws the outline of every page intersecting `visible`, one device pixel wide.
fn draw_page_borders(
    engine: &Engine,
    cx: &cairo::Context,
    doc_bounds: Aabb,
    visible: Aabb,
    zoom: f64,
) -> anyhow::Result<()> {
    let format = &engine.document.config.format;
    if !format.show_borders {
        return Ok(());
    }
    cx.save()?;
    // A little slack so the outer borders, which sit on the document edge, stay visible.
    let clip = doc_bounds.loosened(2.0 / zoom);
    cx.rectangle(clip.mins.x, clip.mins.y, clip.extents().x, clip.extents().y);
    cx.clip();
    for page in visible.split_extended_origin_aligned(format.size(), SplitOrder::default()) {
        if page.intersects(&visible) {
            cx.rectangle(page.mins.x, page.mins.y, page.extents().x, page.extents().y);
        }
    }
    let color = format.border_color;
    cx.set_source_rgba(color.r, color.g, color.b, color.a);
    cx.set_line_width(1.0 / zoom);
    cx.stroke()?;
    cx.restore()?;
    Ok(())
}

/// Converts cairo's `ARGB32` (native-endian, so BGRA bytes on little-endian targets) into
/// RGBA bytes. Both are premultiplied.
fn copy_bgra_to_rgba(src: &[u8], src_stride: usize, out: &mut [u8], layout: PixelLayout) {
    let row_len = layout.width as usize * BYTES_PER_PIXEL;
    for row in 0..layout.height as usize {
        let src_row = &src[row * src_stride..][..row_len];
        let out_row = &mut out[row * layout.stride..][..row_len];
        let (src_pixels, _) = src_row.as_chunks::<BYTES_PER_PIXEL>();
        let (out_pixels, _) = out_row.as_chunks_mut::<BYTES_PER_PIXEL>();
        for (src_pixel, out_pixel) in src_pixels.iter().zip(out_pixels) {
            let [alpha, red, green, blue] = u32::from_ne_bytes(*src_pixel).to_be_bytes();
            *out_pixel = [red, green, blue, alpha];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_red_and_blue_channels() {
        let pixel = u32::from_be_bytes([0x80, 0x10, 0x20, 0x30]).to_ne_bytes();
        let layout = PixelLayout { width: 1, height: 1, stride: BYTES_PER_PIXEL };
        let mut out = [0_u8; 4];

        copy_bgra_to_rgba(&pixel, BYTES_PER_PIXEL, &mut out, layout);

        assert_eq!(out, [0x10, 0x20, 0x30, 0x80]);
    }

    #[test]
    fn min_len_ignores_padding_after_last_row() {
        let layout = PixelLayout { width: 2, height: 3, stride: 12 };

        assert_eq!(layout.min_len(), 12 * 2 + 2 * BYTES_PER_PIXEL);
    }
}
