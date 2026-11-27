//! PDF pages and pictures under the ink are drawn from rasters kept between frames.

use crate::image_layer::ImageLayer;
use crate::render::BYTES_PER_PIXEL;
use rnote_library::creation::PdfPages;

use super::*;

const PDF: &[u8] = include_bytes!("../../tests/fixtures/tres_paginas.pdf");

fn with_pdf(vector: bool) -> RnoteDocument {
    let mut document = plain_document();
    document.set_layout(Layout::FixedSize);
    let format = document.engine.document.config.format;
    let mut pdf =
        PdfPages::new(PDF.to_vec(), None, 0..3, vector, format).expect("the test PDF opens");
    while pdf.import_next().expect("its pages read") {}
    document.insert_page_strokes(0, pdf.into_pages());
    draw_line(&mut document, 300.0);
    document
}

fn render(document: &mut RnoteDocument, viewport: Viewport) -> Vec<u8> {
    let layout = PixelLayout { width: 600, height: 800, stride: 600 * BYTES_PER_PIXEL };
    let mut out = vec![0_u8; layout.min_len()];
    document.render_rgba(viewport, layout, &mut out).expect("renders");
    out
}

const VIEW: Viewport = Viewport { x: 0.0, y: 0.0, zoom: 0.5 };

#[test]
fn pdf_pages_are_rasterized_once_for_many_frames() {
    for vector in [true, false] {
        let mut document = with_pdf(vector);

        render(&mut document, VIEW);
        let first = document.images.rasterizations();
        render(&mut document, Viewport { y: 40.0, ..VIEW });
        render(&mut document, VIEW);

        assert!(first > 0, "nothing was rasterized");
        assert_eq!(document.images.rasterizations(), first, "vector: {vector}");
    }
}

#[test]
fn another_zoom_rasterizes_again() {
    let mut document = with_pdf(true);
    render(&mut document, VIEW);
    let first = document.images.rasterizations();

    render(&mut document, Viewport { zoom: 1.5, ..VIEW });

    assert!(document.images.rasterizations() > first);
}

#[test]
fn cached_pages_look_like_pages_drawn_directly() {
    for vector in [true, false] {
        let mut document = with_pdf(vector);
        let cached = render(&mut document, VIEW);
        document.images = ImageLayer::with_budget(0);
        let direct = render(&mut document, VIEW);

        let differences: Vec<u32> =
            cached.iter().zip(&direct).map(|(a, b)| u32::from(a.abs_diff(*b))).collect();
        let mean = f64::from(differences.iter().sum::<u32>()) / differences.len() as f64;
        assert!(mean < 1.0, "vector: {vector}, mean difference {mean}");
    }
}

#[test]
fn bitmap_pages_stay_cached_however_far_one_zooms_in() {
    let mut document = with_pdf(false);
    let close_up = Viewport { x: 100.0, y: 100.0, zoom: 12.0 };
    render(&mut document, close_up);
    let first = document.images.rasterizations();

    render(&mut document, Viewport { x: 140.0, ..close_up });

    assert!(first > 0, "the bitmap page was drawn from its source");
    assert_eq!(document.images.rasterizations(), first);
}

/// A 40 × 20 photo whose EXIF data says to turn it a quarter: upright it is 20 × 40.
const TURNED_PHOTO: &[u8] = include_bytes!("../../tests/fixtures/girada.jpg");

#[test]
fn a_pasted_photo_stands_as_its_camera_says() {
    let mut document = plain_document();

    document.paste_image(TURNED_PHOTO.to_vec(), Vector2::new(300.0, 300.0)).expect("pastes");

    let bounds = document.selection_bounds().expect("the photo is selected");
    let size = bounds.extents();
    assert!(size[1] > size[0], "lies on its side: {size:?}");
}
