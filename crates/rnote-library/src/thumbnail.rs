//! Thumbnails of notebooks, rendered with the engine.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;

use anyhow::Context;
use futures::executor::block_on;
use rnote_compose::SplitOrder;
use rnote_engine::Engine;
use rnote_engine::engine::EngineSnapshot;

use crate::Notebook;

/// A PNG of the notebook's first page with content (the first page of an empty one), at
/// most `size_px` wide and high.
pub fn render_thumbnail(rnote_bytes: Vec<u8>, size_px: u32) -> anyhow::Result<Vec<u8>> {
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

/// The cache key of `notebook`'s thumbnail: the same while its path, modification time and
/// size don't change.
#[must_use]
pub fn thumbnail_key(notebook: &Notebook) -> String {
    let mut hasher = DefaultHasher::new();
    (&notebook.path, notebook.modified_ms, notebook.size).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
