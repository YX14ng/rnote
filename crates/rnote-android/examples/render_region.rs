//! Renders a region of an `.rnote` file to raw RGBA, for debugging the viewer on the host.
//!
//! Usage: `render_region <file.rnote> <x> <y> <zoom> <width> <height> <out.rgba>`
//!
//! Prints the document bounds. Convert the output with
//! `magick -size <width>x<height> -depth 8 rgba:<out.rgba> out.png`.

use std::env;
use std::fs;

use anyhow::Context;
use rnote_android::{BYTES_PER_PIXEL, PixelLayout, RnoteDocument, Viewport};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let [_, file, x, y, zoom, width, height, out] = args.as_slice() else {
        anyhow::bail!(
            "usage: render_region <file.rnote> <x> <y> <zoom> <width> <height> <out.rgba>"
        );
    };
    let mut document = RnoteDocument::try_open(fs::read(file).context("Reading the file failed")?)?;
    let bounds = document.bounds();
    println!("bounds: {:?} .. {:?}", bounds.mins, bounds.maxs);

    let width: u32 = width.parse()?;
    let height: u32 = height.parse()?;
    let layout = PixelLayout { width, height, stride: width as usize * BYTES_PER_PIXEL };
    let viewport = Viewport { x: x.parse()?, y: y.parse()?, zoom: zoom.parse()? };
    let mut pixels = vec![0_u8; layout.min_len()];
    document.render_rgba(viewport, layout, &mut pixels)?;
    fs::write(out, pixels).context("Writing the output failed")?;
    Ok(())
}
