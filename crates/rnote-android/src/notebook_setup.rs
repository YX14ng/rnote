//! A new notebook made from what the "new notebook" screen chose: page size, how pages
//! grow, paper color, template and spacing, optionally with an image on its first page.

use std::io::Cursor;

use anyhow::Context;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use parry2d_f64::math::Vector2;
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::strokes::resize::ImageSizeOption;
use rnote_engine::strokes::{BitmapImage, Stroke};
use rnote_engine::{Engine, WidgetFlags};
use serde::{Deserialize, Serialize};

use crate::android_color::color_from_argb;
use crate::document_settings::{apply_background, apply_layout, apply_page_size};

const MM_PER_INCH: f64 = 25.4;
/// The longest side an inserted image keeps, in pixels. The engine keeps images as raw
/// pixels (4 bytes each) inside the notebook, so this bounds what a photo weighs there and
/// in Drive: about 7.7 MB before compression, instead of 48 MB for a 12 MP photo.
const MAX_IMAGE_SIDE: u32 = 1600;

/// How the pages of a new notebook grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SetupLayout {
    /// Sheets of the chosen size.
    FixedSize,
    /// One long page downwards.
    ContinuousVertical,
    /// Grows in every direction.
    Infinite,
}

/// The template drawn on the paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SetupPattern {
    /// Blank.
    None,
    /// Ruled lines.
    Lines,
    /// Squares.
    Grid,
    /// Dots.
    Dots,
}

/// What a new notebook starts with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct NotebookSetup {
    /// Page width in millimeters.
    pub page_width_mm: f64,
    /// Page height in millimeters.
    pub page_height_mm: f64,
    /// How the pages grow.
    pub layout: SetupLayout,
    /// Paper color, as ARGB.
    pub background_argb: u32,
    /// Template.
    pub pattern: SetupPattern,
    /// Template color, as ARGB.
    pub pattern_argb: u32,
    /// Template spacing in millimeters.
    pub pattern_size_mm: f64,
    /// Cover color in the library, as ARGB, or `None` to show the first page.
    pub cover: Option<u32>,
}

impl Default for NotebookSetup {
    /// A4 portrait sheets, dark paper with a 5 mm grid, no cover.
    fn default() -> Self {
        Self {
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            layout: SetupLayout::FixedSize,
            background_argb: 0xFF28_2A36,
            pattern: SetupPattern::Grid,
            pattern_argb: 0xFF62_72A4,
            pattern_size_mm: 5.0,
            cover: None,
        }
    }
}

/// An engine with an empty document set up as `setup` says.
pub(crate) fn engine_with_setup(setup: &NotebookSetup) -> Engine {
    let mut engine = Engine::default();
    let _flags: WidgetFlags =
        apply_page_size(&mut engine, setup.page_width_mm, setup.page_height_mm);
    let pattern_size = setup.pattern_size_mm / MM_PER_INCH * engine.document.config.format.dpi();
    apply_background(
        &mut engine,
        color_from_argb(setup.background_argb as i32),
        pattern_style(setup.pattern),
        color_from_argb(setup.pattern_argb as i32),
        pattern_size,
    );
    let _flags = apply_layout(&mut engine, layout(setup.layout));
    engine
}

/// The `.rnote` bytes of an empty notebook set up as `setup` says.
pub(crate) fn new_notebook_bytes(
    setup: &NotebookSetup,
    file_name: &str,
) -> anyhow::Result<Vec<u8>> {
    save(&engine_with_setup(setup), file_name)
}

/// The `.rnote` bytes of a notebook set up as `setup` says with the image `image` (PNG,
/// JPEG, ...) at the top of its first page, across the page width. Photos are turned
/// upright as their EXIF data says and scaled down to [`MAX_IMAGE_SIDE`].
pub(crate) fn image_notebook_bytes(
    setup: &NotebookSetup,
    image: &[u8],
    file_name: &str,
) -> anyhow::Result<Vec<u8>> {
    let png = upright_png(image)?;
    let mut engine = engine_with_setup(setup);
    let width = engine.document.config.format.width();
    let (pixel_width, pixel_height) = png.1;
    let size = Vector2::new(width, width * f64::from(pixel_height) / f64::from(pixel_width));
    let image =
        BitmapImage::from_image_bytes(&png.0, Vector2::ZERO, ImageSizeOption::ImposeSize(size))?;
    let _flags = engine.import_generated_content(
        vec![(Stroke::BitmapImage(image), None)],
        /* adjust_document */ false,
    );
    let _flags = engine.deselect_all_strokes();
    save(&engine, file_name)
}

/// `image` decoded, turned upright, scaled down to [`MAX_IMAGE_SIDE`] and encoded as PNG,
/// with its size in pixels.
fn upright_png(image: &[u8]) -> anyhow::Result<(Vec<u8>, (u32, u32))> {
    let mut decoder = ImageReader::new(Cursor::new(image))
        .with_guessed_format()?
        .into_decoder()
        .context("The image can't be read")?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder).context("The image can't be read")?;
    image.apply_orientation(orientation);
    if image.width().max(image.height()) > MAX_IMAGE_SIDE {
        image = image.resize(MAX_IMAGE_SIDE, MAX_IMAGE_SIDE, FilterType::Triangle);
    }
    let mut png = Vec::new();
    image.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)?;
    Ok((png, (image.width(), image.height())))
}

fn save(engine: &Engine, file_name: &str) -> anyhow::Result<Vec<u8>> {
    futures::executor::block_on(engine.save_as_rnote_bytes(file_name.to_owned()))
        .context("The save task was cancelled")?
}

fn layout(layout: SetupLayout) -> Layout {
    match layout {
        SetupLayout::FixedSize => Layout::FixedSize,
        SetupLayout::ContinuousVertical => Layout::ContinuousVertical,
        SetupLayout::Infinite => Layout::Infinite,
    }
}

fn pattern_style(pattern: SetupPattern) -> PatternStyle {
    match pattern {
        SetupPattern::None => PatternStyle::None,
        SetupPattern::Lines => PatternStyle::Lines,
        SetupPattern::Grid => PatternStyle::Grid,
        SetupPattern::Dots => PatternStyle::Dots,
    }
}

#[cfg(test)]
mod tests;
