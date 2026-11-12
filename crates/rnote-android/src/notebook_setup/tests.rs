#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, RgbImage};
use rnote_compose::shapes::Shapeable;
use rnote_engine::Engine;
use rnote_engine::document::Layout;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::engine::EngineSnapshot;
use rnote_engine::strokes::{BitmapImage, Stroke};

use super::{NotebookSetup, SetupLayout, SetupPattern, image_notebook_bytes, new_notebook_bytes};
use crate::document_settings::{DocumentSettings, document_settings};

const MM_PER_INCH: f64 = 25.4;

fn reopen(bytes: Vec<u8>) -> (Engine, Vec<BitmapImage>) {
    let snapshot =
        futures::executor::block_on(EngineSnapshot::load_from_rnote_bytes(bytes)).unwrap();
    let images = snapshot
        .stroke_components
        .values()
        .filter_map(|stroke| match stroke.as_ref() {
            Stroke::BitmapImage(image) => Some(image.clone()),
            _ => None,
        })
        .collect();
    let mut engine = Engine::default();
    let _flags = engine.load_snapshot(snapshot);
    (engine, images)
}

fn settings_of(bytes: Vec<u8>) -> DocumentSettings {
    document_settings(&reopen(bytes).0)
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() < tolerance
}

fn encoded(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), format).unwrap();
    bytes
}

/// A JPEG of `width`×`height` with an EXIF orientation tag inserted after its SOI marker.
fn jpeg_with_orientation(width: u32, height: u32, orientation: u8) -> Vec<u8> {
    let jpeg = encoded(&DynamicImage::ImageRgb8(RgbImage::new(width, height)), ImageFormat::Jpeg);
    let tiff: [u8; 26] = [
        b'M',
        b'M',
        0,
        42,
        0,
        0,
        0,
        8, // Big-endian TIFF header, first IFD at 8.
        0,
        1, // One entry:
        0x01,
        0x12,
        0,
        3,
        0,
        0,
        0,
        1,
        0,
        orientation,
        0,
        0, // Orientation, SHORT, 1 value.
        0,
        0,
        0,
        0, // No next IFD.
    ];
    let mut app1 = vec![0xFF, 0xE1, 0, 34];
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(&tiff);
    let mut bytes = jpeg[..2].to_vec();
    bytes.extend_from_slice(&app1);
    bytes.extend_from_slice(&jpeg[2..]);
    bytes
}

#[test]
fn default_setup_is_a4_dark_grid_5mm() {
    let settings = settings_of(new_notebook_bytes(&NotebookSetup::default(), "x").unwrap());

    assert!(close(settings.page_width_mm, 210.0, 0.01), "{settings:?}");
    assert!(close(settings.page_height_mm, 297.0, 0.01), "{settings:?}");
    assert_eq!(settings.layout, Layout::FixedSize);
    assert_eq!(settings.pattern, PatternStyle::Grid);
    let dpi = settings.page_width / settings.page_width_mm * MM_PER_INCH;
    assert!(close(settings.pattern_size, 5.0 / MM_PER_INCH * dpi, 0.01), "{settings:?}");
    assert_eq!(crate::android_color::color_to_argb(settings.background_color) as u32, 0xFF28_2A36);
}

#[test]
fn setup_is_applied() {
    let setup = NotebookSetup {
        page_width_mm: 279.4,
        page_height_mm: 215.9,
        layout: SetupLayout::Infinite,
        background_argb: 0xFFFB_F6E9,
        pattern: SetupPattern::Lines,
        pattern_argb: 0xFFC8_CCD6,
        pattern_size_mm: 8.0,
        cover: Some(0xFF3F_8F6B),
    };

    let settings = settings_of(new_notebook_bytes(&setup, "x").unwrap());

    assert!(close(settings.page_width_mm, 279.4, 0.01), "{settings:?}");
    assert!(close(settings.page_height_mm, 215.9, 0.01), "{settings:?}");
    assert_eq!(settings.layout, Layout::Infinite);
    assert_eq!(settings.pattern, PatternStyle::Lines);
    assert_eq!(crate::android_color::color_to_argb(settings.background_color) as u32, 0xFFFB_F6E9);
    assert_eq!(crate::android_color::color_to_argb(settings.pattern_color) as u32, 0xFFC8_CCD6);
    let dpi = settings.page_width / settings.page_width_mm * MM_PER_INCH;
    assert!(close(settings.pattern_size, 8.0 / MM_PER_INCH * dpi, 0.01), "{settings:?}");
}

#[test]
fn image_notebook_puts_the_image_at_the_top_full_width() {
    let png = encoded(&DynamicImage::ImageRgb8(RgbImage::new(400, 200)), ImageFormat::Png);

    let bytes = image_notebook_bytes(&NotebookSetup::default(), &png, "x").unwrap();

    let (engine, images) = reopen(bytes);
    assert_eq!(images.len(), 1);
    let bounds = images[0].bounds();
    let width = engine.document.config.format.width();
    assert!(close(bounds.mins.x, 0.0, 0.5) && close(bounds.mins.y, 0.0, 0.5), "{bounds:?}");
    assert!(close(bounds.extents().x, width, 0.5), "{bounds:?}");
    assert!(close(bounds.extents().y, width / 2.0, 0.5), "{bounds:?}");
}

#[test]
fn photo_is_turned_upright() {
    let jpeg = jpeg_with_orientation(40, 20, 6);

    let (_, images) = reopen(image_notebook_bytes(&NotebookSetup::default(), &jpeg, "x").unwrap());

    let image = &images[0].image;
    assert_eq!((image.pixel_width, image.pixel_height), (20, 40));
    let bounds = images[0].bounds();
    assert!(bounds.extents().y > bounds.extents().x, "{bounds:?}");
}

#[test]
fn big_photo_is_scaled_down() {
    let jpeg = encoded(&DynamicImage::ImageRgb8(RgbImage::new(4000, 3000)), ImageFormat::Jpeg);

    let (_, images) = reopen(image_notebook_bytes(&NotebookSetup::default(), &jpeg, "x").unwrap());

    let image = &images[0].image;
    assert_eq!((image.pixel_width, image.pixel_height), (2400, 1800));
}

#[test]
fn something_that_is_not_an_image_is_an_error() {
    assert!(image_notebook_bytes(&NotebookSetup::default(), b"nope", "x").is_err());
}
