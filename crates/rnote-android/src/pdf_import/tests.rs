#![allow(clippy::unwrap_used)]

use parry2d_f64::bounding_volume::Aabb;
use rnote_compose::shapes::Shapeable;
use rnote_engine::Engine;
use rnote_engine::document::Layout;
use rnote_engine::engine::EngineSnapshot;
use rnote_engine::strokes::Stroke;

use super::{PdfImport, PdfOpenError, pdf_page_count};
use crate::notebook_setup::NotebookSetup;

const PLAIN: &[u8] = include_bytes!("../tests/fixtures/tres_paginas.pdf");
const ENCRYPTED: &[u8] = include_bytes!("../tests/fixtures/con_clave.pdf");

/// The kinds and bounds of the imported pages, top to bottom, and the reopened engine.
fn reopen(bytes: Vec<u8>) -> (Engine, Vec<(&'static str, Aabb)>) {
    let snapshot =
        futures::executor::block_on(EngineSnapshot::load_from_rnote_bytes(bytes)).unwrap();
    let mut pages: Vec<_> = snapshot
        .stroke_components
        .values()
        .map(|stroke| {
            let kind = match stroke.as_ref() {
                Stroke::VectorImage(_) => "vector",
                Stroke::BitmapImage(_) => "bitmap",
                _ => "other",
            };
            (kind, stroke.bounds())
        })
        .collect();
    pages.sort_by(|a, b| a.1.mins.y.total_cmp(&b.1.mins.y));
    let mut engine = Engine::default();
    let _flags = engine.load_snapshot(snapshot);
    (engine, pages)
}

fn import_all(mut import: PdfImport) -> Vec<u8> {
    while import.import_next().unwrap() {}
    import.finish("x").unwrap()
}

fn plain(pages: std::ops::Range<usize>, vector: bool) -> PdfImport {
    PdfImport::new(PLAIN.to_vec(), None, &NotebookSetup::default(), pages, vector).unwrap()
}

#[test]
fn page_count_of_a_plain_pdf() {
    assert_eq!(pdf_page_count(PLAIN, None).unwrap(), 3);
}

#[test]
fn encrypted_pdf_needs_a_password() {
    assert!(matches!(pdf_page_count(ENCRYPTED, None), Err(PdfOpenError::NeedsPassword)));
}

#[test]
fn wrong_password_needs_a_password() {
    assert!(matches!(pdf_page_count(ENCRYPTED, Some("otra")), Err(PdfOpenError::NeedsPassword)));
}

#[test]
fn right_password_opens() {
    assert_eq!(pdf_page_count(ENCRYPTED, Some("clave")).unwrap(), 3);
    let import = PdfImport::new(
        ENCRYPTED.to_vec(),
        Some("clave".to_owned()),
        &NotebookSetup::default(),
        0..3,
        true,
    )
    .unwrap();
    assert_eq!(reopen(import_all(import)).1.len(), 3);
}

#[test]
fn garbage_is_invalid() {
    assert!(matches!(pdf_page_count(b"no soy un pdf", None), Err(PdfOpenError::Invalid(_))));
}

#[test]
fn import_reports_each_page_then_finishes() {
    let mut import = plain(0..3, true);
    assert_eq!(import.total(), 3);

    let steps: Vec<bool> = (0..4).map(|_| import.import_next().unwrap()).collect();

    assert_eq!(steps, [true, true, true, false]);
    let (_, pages) = reopen(import.finish("x").unwrap());
    assert_eq!(pages.iter().map(|page| page.0).collect::<Vec<_>>(), ["vector"; 3]);
}

#[test]
fn bitmap_import_makes_bitmap_images() {
    let (_, pages) = reopen(import_all(plain(0..3, false)));

    assert_eq!(pages.iter().map(|page| page.0).collect::<Vec<_>>(), ["bitmap"; 3]);
}

#[test]
fn mixed_page_sizes_fill_the_width_one_per_page() {
    let (engine, pages) = reopen(import_all(plain(0..3, true)));

    let format = &engine.document.config.format;
    let (width, height) = (format.width(), format.height());
    assert_eq!(engine.document.config.layout, Layout::FixedSize);
    assert!((height - width * 842.0 / 595.0).abs() < 0.5, "{width} x {height}");
    for (i, (_, bounds)) in pages.iter().enumerate() {
        assert!((bounds.extents().x - width).abs() < 0.5, "page {i}: {bounds:?}");
        assert!((bounds.mins.y - i as f64 * height).abs() < 0.5, "page {i}: {bounds:?}");
        assert!(bounds.maxs.y <= (i + 1) as f64 * height + 0.5, "page {i}: {bounds:?}");
    }
    let landscape = pages[1].1.extents();
    assert!((landscape.y - width * 595.0 / 842.0).abs() < 0.5, "{landscape:?}");
}

#[test]
fn page_range_imports_only_those_pages() {
    let (_, pages) = reopen(import_all(plain(1..2, true)));

    assert_eq!(pages.len(), 1);
    assert!(pages[0].1.extents().x > pages[0].1.extents().y, "the landscape page");
}

#[test]
fn page_range_outside_the_pdf_is_invalid() {
    let setup = NotebookSetup::default();
    for pages in [2..5, 1..1] {
        let result = PdfImport::new(PLAIN.to_vec(), None, &setup, pages, true);
        assert!(matches!(result, Err(PdfOpenError::Invalid(_))));
    }
}
