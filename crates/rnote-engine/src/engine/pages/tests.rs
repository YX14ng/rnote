//! The engine's own view of the pages of a document of sheets.

use std::time::Instant;

use p2d::math::Vector2;
use rnote_compose::Style;
use rnote_compose::shapes::{Line, Shape};

use crate::Engine;
use crate::document::Layout;
use crate::engine::export::DocExportPrefs;
use crate::strokes::{ShapeStroke, Stroke};

/// An engine with a document of `pages` sheets and a line in the middle of each sheet in
/// `lines`, and the height of a sheet.
fn sheets(pages: u32, lines: &[u32]) -> (Engine, f64) {
    let mut engine = Engine::default();
    let _ = engine.set_doc_layout(Layout::FixedSize);
    if pages > 1 {
        let _ = engine.doc_insert_pages_fixed_size(1, pages - 1);
    }
    let height = engine.document.config.format.height();
    for &page in lines {
        draw_line(&mut engine, (f64::from(page) + 0.5) * height);
    }
    (engine, height)
}

/// Adds a line across the sheet at height `y`, one step of the history like a pen stroke.
fn draw_line(engine: &mut Engine, y: f64) {
    let line = Line {
        start: Vector2::new(100.0, y),
        end: Vector2::new(300.0, y),
    };
    let stroke = Stroke::ShapeStroke(ShapeStroke::new(Shape::Line(line), Style::default()));
    let _ = engine.store.insert_stroke(stroke, None);
    let _ = engine.store.record(Instant::now());
}

#[test]
fn exporting_keeps_the_blank_pages_only_when_asked() {
    let (engine, _) = sheets(3, &[1]);
    let mut prefs = DocExportPrefs::default();

    assert_eq!(engine.export_pages_content(&prefs).len(), 1);
    prefs.with_empty_pages = true;
    assert_eq!(engine.export_pages_content(&prefs).len(), 3);
}

#[test]
fn undo_brings_back_the_sheet_of_a_removed_page() {
    let (mut engine, _) = sheets(3, &[2]);
    let _ = engine.doc_remove_pages_fixed_size(&[2]);
    assert_eq!(engine.doc_page_count_fixed_size(), Some(2));

    let _ = engine.undo(Instant::now());

    assert_eq!(engine.doc_page_count_fixed_size(), Some(3));
}

#[test]
fn tells_the_page_at_a_height() {
    let (engine, height) = sheets(3, &[]);

    assert_eq!(engine.doc_page_at_fixed_size(0.0), Some(0));
    assert_eq!(engine.doc_page_at_fixed_size(1.5 * height), Some(1));
    assert_eq!(engine.doc_page_at_fixed_size(99.0 * height), Some(2));
    assert_eq!(engine.doc_page_at_fixed_size(-5.0), Some(0));
    assert_eq!(engine.doc_page_top_fixed_size(2), Some(2.0 * height));
}

#[test]
fn a_page_fingerprint_changes_only_where_something_was_drawn() {
    let (mut engine, height) = sheets(3, &[0, 2]);
    let before = engine.doc_page_fingerprints_fixed_size();

    draw_line(&mut engine, 1.5 * height);
    let after = engine.doc_page_fingerprints_fixed_size();

    assert_eq!(after.len(), 3);
    assert_eq!(after[0], before[0]);
    assert_ne!(after[1], before[1]);
    assert_eq!(after[2], before[2]);
}
