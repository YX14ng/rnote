//! Rendering of an `.rnote` document into RGBA pixel buffers.

use anyhow::{Context, bail};
use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_engine::Engine;
use rnote_engine::engine::EngineSnapshot;

/// Color of the area around the document, as `[r, g, b]` in `0.0..=1.0`.
pub const DESK_COLOR: [f64; 3] = [0.24, 0.24, 0.26];

/// Bytes per pixel of the RGBA buffers written by [`DocumentView::render_rgba`].
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

/// Size and layout of a pixel buffer that [`DocumentView::render_rgba`] writes into.
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

/// A loaded, read-only Rnote document that can render any region of itself.
pub struct DocumentView {
    engine: Engine,
}

impl DocumentView {
    /// Loads a document from the contents of an `.rnote` file.
    pub fn try_new(rnote_bytes: Vec<u8>) -> anyhow::Result<Self> {
        let snapshot =
            futures::executor::block_on(EngineSnapshot::load_from_rnote_bytes(rnote_bytes))
                .context("Loading the .rnote file failed")?;
        let mut engine = Engine::default();
        // The widget flags only matter for the GTK frontend.
        let _widget_flags = engine.load_snapshot(snapshot);
        Ok(Self { engine })
    }

    /// Bounds of the document in document coordinates.
    ///
    /// These are the pages that have content, extended to the page format.
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        self.engine.bounds_w_content_extended().unwrap_or_else(|| {
            let document = &self.engine.document;
            Aabb::new(
                Vector2::new(document.x, document.y),
                Vector2::new(document.x + document.width, document.y + document.height),
            )
        })
    }

    /// Renders the region of the document given by `viewport` into `out`.
    ///
    /// `out` receives premultiplied RGBA pixels laid out as described by `layout`, which
    /// is what Android's `Bitmap.Config.ARGB_8888` uses. Areas outside of the document
    /// are filled with [`DESK_COLOR`].
    pub fn render_rgba(
        &self,
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

        let mut surface = self.draw_to_surface(viewport, layout.width, layout.height)?;
        let src_stride = usize::try_from(surface.stride()).context("Negative cairo stride")?;
        let src = surface.data().context("Accessing the cairo surface data failed")?;
        copy_bgra_to_rgba(&src, src_stride, out, layout);
        Ok(())
    }

    fn draw_to_surface(
        &self,
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
        if let Some(visible) = viewport_bounds.intersection(&self.bounds()) {
            cx.scale(viewport.zoom, viewport.zoom);
            cx.translate(-viewport.x, -viewport.y);
            self.engine.extract_content_in_bounds(visible).draw_to_cairo(
                &cx,
                /* draw_background */ true,
                /* draw_pattern */ true,
                /* optimize_printing */ false,
                /* margin */ 0.0,
                /* image_scale */ viewport.zoom,
            )?;
        }

        drop(cx);
        surface.flush();
        Ok(surface)
    }
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
    use rnote_engine::document::background::PatternStyle;

    /// Builds the bytes of an empty document with a plain background of the given color.
    fn empty_document_bytes(red: f64, green: f64, blue: f64) -> Vec<u8> {
        let mut engine = Engine::default();
        let background = &mut engine.document.config.background;
        background.pattern = PatternStyle::None;
        background.color.r = red;
        background.color.g = green;
        background.color.b = blue;
        background.color.a = 1.0;
        futures::executor::block_on(engine.save_as_rnote_bytes("test.rnote".to_owned()))
            .expect("the save task should not be cancelled")
            .expect("saving an empty document should succeed")
    }

    fn render_pixel(view: &DocumentView, x: f64, y: f64) -> [u8; 4] {
        let layout = PixelLayout { width: 1, height: 1, stride: BYTES_PER_PIXEL };
        let mut out = [0_u8; 4];
        view.render_rgba(Viewport { x, y, zoom: 1.0 }, layout, &mut out)
            .expect("rendering one pixel should succeed");
        out
    }

    fn to_byte(channel: f64) -> u8 {
        (channel * 255.0).round() as u8
    }

    #[test]
    fn rejects_invalid_bytes() {
        assert!(DocumentView::try_new(b"not an rnote file".to_vec()).is_err());
    }

    #[test]
    fn renders_background_inside_document() {
        let view = DocumentView::try_new(empty_document_bytes(1.0, 0.5, 0.0))
            .expect("an empty document should load");
        let center = view.bounds().center();

        let pixel = render_pixel(&view, center.x, center.y);

        let expected = [to_byte(1.0), to_byte(0.5), to_byte(0.0), 255];
        for (channel, (got, want)) in pixel.iter().zip(expected).enumerate() {
            assert!(got.abs_diff(want) <= 1, "channel {channel}: got {pixel:?}, want {expected:?}");
        }
    }

    #[test]
    fn renders_desk_outside_document() {
        let view = DocumentView::try_new(empty_document_bytes(1.0, 1.0, 1.0))
            .expect("an empty document should load");
        let mins = view.bounds().mins;

        let pixel = render_pixel(&view, mins.x - 100.0, mins.y - 100.0);

        let [r, g, b] = DESK_COLOR;
        let expected = [to_byte(r), to_byte(g), to_byte(b), 255];
        for (channel, (got, want)) in pixel.iter().zip(expected).enumerate() {
            assert!(got.abs_diff(want) <= 1, "channel {channel}: got {pixel:?}, want {expected:?}");
        }
    }

    #[test]
    fn rejects_buffer_smaller_than_layout() {
        let view = DocumentView::try_new(empty_document_bytes(1.0, 1.0, 1.0))
            .expect("an empty document should load");
        let layout = PixelLayout { width: 2, height: 2, stride: 2 * BYTES_PER_PIXEL };
        let mut out = vec![0_u8; layout.min_len() - 1];

        let result = view.render_rgba(Viewport { x: 0.0, y: 0.0, zoom: 1.0 }, layout, &mut out);

        assert!(result.is_err(), "a short buffer must be rejected, not written past its end");
    }

    #[test]
    fn swaps_red_and_blue_channels() {
        let pixel = u32::from_be_bytes([0x80, 0x10, 0x20, 0x30]).to_ne_bytes();
        let layout = PixelLayout { width: 1, height: 1, stride: BYTES_PER_PIXEL };
        let mut out = [0_u8; 4];

        copy_bgra_to_rgba(&pixel, BYTES_PER_PIXEL, &mut out, layout);

        assert_eq!(out, [0x10, 0x20, 0x30, 0x80]);
    }
}
