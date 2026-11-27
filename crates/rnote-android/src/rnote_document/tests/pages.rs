//! Pages of a document of sheets: counting, removing and inserting them.

use parry2d_f64::bounding_volume::Aabb;
use rnote_compose::shapes::Shapeable;
use rnote_engine::strokes::Stroke;

use super::*;
use rnote_library::creation::PdfPages;

const PDF: &[u8] = include_bytes!("../../tests/fixtures/tres_paginas.pdf");

/// A document of `pages` sheets with a line in the middle of each sheet in `lines`.
fn sheets(pages: u32, lines: &[u32]) -> (RnoteDocument, f64) {
    let mut document = plain_document();
    document.set_layout(Layout::FixedSize);
    if pages > 1 {
        document.insert_blank_pages(1, pages - 1);
    }
    let height = document.engine.document.config.format.height();
    for &page in lines {
        draw_line(&mut document, (f64::from(page) + 0.5) * height);
    }
    (document, height)
}

/// The vertical middle of every stroke, top to bottom.
fn stroke_middles(document: &RnoteDocument) -> Vec<f64> {
    let everything = Aabb::new(Vector2::splat(-1e6), Vector2::splat(1e6));
    let content = document.engine.extract_content_in_bounds(everything);
    let mut middles: Vec<f64> =
        content.strokes.iter().map(|layered| layered.stroke.bounds().center().y).collect();
    middles.sort_by(f64::total_cmp);
    middles
}

fn assert_middles(document: &RnoteDocument, height: f64, pages: &[f64]) {
    let got = stroke_middles(document);
    let want: Vec<f64> = pages.iter().map(|page| page * height).collect();
    assert_eq!(got.len(), want.len(), "got {got:?}, want {want:?}");
    for (got_y, want_y) in got.iter().zip(&want) {
        assert!((got_y - want_y).abs() < 1.0, "got {got:?}, want {want:?}");
    }
}

#[test]
fn only_documents_of_sheets_have_pages() {
    let mut document = plain_document();
    document.set_layout(Layout::Infinite);
    assert_eq!(document.page_count(), None);

    let (document, _) = sheets(3, &[]);
    assert_eq!(document.page_count(), Some(3));
}

#[test]
fn tells_which_pages_have_something_on_them() {
    let (document, _) = sheets(3, &[0, 2]);

    assert_eq!(document.pages_with_content(), [true, false, true]);
}

#[test]
fn removing_a_page_takes_what_is_on_it_and_moves_the_rest_up() {
    let (mut document, height) = sheets(3, &[0, 1, 2]);

    let state = document.remove_pages(&[1]);

    assert!(state.modified && state.can_undo && state.bounds_changed, "got {state:?}");
    assert_eq!(document.page_count(), Some(2));
    assert_middles(&document, height, &[0.5, 1.5]);
}

#[test]
fn removing_several_pages_at_once() {
    let (mut document, height) = sheets(5, &[0, 1, 2, 3, 4]);

    document.remove_pages(&[3, 1]);

    assert_eq!(document.page_count(), Some(3));
    assert_middles(&document, height, &[0.5, 1.5, 2.5]);
    assert_eq!(document.pages_with_content(), [true, true, true]);
}

#[test]
fn removing_every_page_leaves_one_blank_page() {
    let (mut document, _) = sheets(2, &[0, 1]);

    document.remove_pages(&[0, 1]);

    assert_eq!(document.page_count(), Some(1));
    assert_eq!(document.pages_with_content(), [false]);
}

#[test]
fn undo_brings_a_removed_page_back() {
    let (mut document, height) = sheets(3, &[0, 1, 2]);
    document.remove_pages(&[1]);

    document.undo();

    assert_eq!(document.page_count(), Some(3));
    assert_middles(&document, height, &[0.5, 1.5, 2.5]);
}

#[test]
fn inserting_blank_pages_moves_what_follows_down() {
    let (mut document, height) = sheets(2, &[0, 1]);

    let state = document.insert_blank_pages(1, 2);

    assert!(state.modified && state.can_undo, "got {state:?}");
    assert_eq!(document.page_count(), Some(4));
    assert_middles(&document, height, &[0.5, 3.5]);
}

#[test]
fn inserting_a_pdf_puts_each_of_its_pages_on_a_sheet_after_the_chosen_one() {
    let (mut document, height) = sheets(2, &[0, 1]);
    let format = document.engine.document.config.format;
    let mut pdf =
        PdfPages::new(PDF.to_vec(), None, 0..3, true, format).expect("the test PDF opens");
    while pdf.import_next().expect("its pages read") {}

    let state = document.insert_page_strokes(1, pdf.into_pages());

    assert!(state.modified && state.can_undo, "got {state:?}");
    assert_eq!(document.page_count(), Some(5));
    assert_eq!(document.pages_with_content(), [true; 5]);
    let everything = Aabb::new(Vector2::splat(-1e6), Vector2::splat(1e6));
    let content = document.engine.extract_content_in_bounds(everything);
    let mut tops: Vec<f64> = content
        .strokes
        .iter()
        .filter(|layered| matches!(*layered.stroke, Stroke::VectorImage(_)))
        .map(|layered| layered.stroke.bounds().mins.y)
        .collect();
    tops.sort_by(f64::total_cmp);
    assert_eq!(tops.len(), 3);
    for (i, top) in tops.iter().enumerate() {
        assert!((top - (1.0 + i as f64) * height).abs() < 1.0, "got {tops:?}");
    }
    // The second sheet's line went below the PDF.
    assert_eq!(stroke_middles(&document).last().map(|y| (y / height).floor()), Some(4.0));
}

#[test]
fn moving_a_page_shifts_the_ones_between() {
    let (mut document, height) = sheets(3, &[2]);

    document.move_page(2, 0);
    assert_middles(&document, height, &[0.5]);

    document.move_page(0, 1);
    assert_middles(&document, height, &[1.5]);
    assert_eq!(document.page_count(), Some(3));
}

#[test]
fn moving_a_page_down_brings_the_next_ones_up() {
    let (mut document, height) = sheets(3, &[0, 1]);

    // The line of the first sheet ends up on the last; the second sheet's moves up.
    document.move_page(0, 2);

    assert_eq!(document.pages_with_content(), [true, false, true]);
    assert_middles(&document, height, &[0.5, 2.5]);
}

#[test]
fn duplicating_a_page_puts_a_copy_right_after_it() {
    let (mut document, height) = sheets(2, &[0, 1]);

    let state = document.duplicate_page(0);

    assert!(state.modified);
    assert_eq!(document.page_count(), Some(3));
    assert_middles(&document, height, &[0.5, 1.5, 2.5]);
}

#[test]
fn exporting_keeps_the_blank_pages_only_when_asked() {
    let (document, _) = sheets(3, &[1]);
    let mut prefs = rnote_engine::engine::export::DocExportPrefs::default();

    assert_eq!(document.engine.export_pages_content(&prefs).len(), 1);
    prefs.with_empty_pages = true;
    assert_eq!(document.engine.export_pages_content(&prefs).len(), 3);
}

#[test]
fn the_engines_own_undo_brings_back_the_sheet_of_a_removed_page() {
    let (mut document, _) = sheets(3, &[2]);
    let _ = document.engine.doc_remove_pages_fixed_size(&[2]);
    assert_eq!(document.page_count(), Some(2));

    let _ = document.engine.undo(std::time::Instant::now());

    assert_eq!(document.page_count(), Some(3));
}
