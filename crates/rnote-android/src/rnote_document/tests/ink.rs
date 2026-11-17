//! The writing of a document, as the ink index reads it.

#![allow(clippy::unwrap_used)]

use parry2d_f64::math::Vector2;

use super::{draw_line, plain_document};

#[test]
fn ink_strokes_reads_pen_strokes_in_drawing_order() {
    let mut document = plain_document();
    draw_line(&mut document, 200.0);
    draw_line(&mut document, 100.0);

    let strokes = document.ink_strokes();

    assert_eq!(strokes.len(), 2);
    assert!(strokes[0].points.iter().all(|&(_, y)| (y - 200.0).abs() < 1.0), "{strokes:?}");
    assert!(strokes[1].points.len() > 2);
    let xs: Vec<f64> = strokes[1].points.iter().map(|&(x, _)| x).collect();
    assert!(xs[0] < xs[xs.len() - 1]);
}

#[test]
fn ink_pending_lists_unread_fragments_and_keeps_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let index = dir.path().join("ink").join("a.json");
    let mut document = plain_document();
    draw_line(&mut document, 200.0);

    let pending: serde_json::Value =
        serde_json::from_str(&document.ink_pending_json(&index, "es").unwrap()).unwrap();

    let fragments = pending.as_array().unwrap();
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0]["hash"].as_str().unwrap().len(), 64);
    assert_eq!(fragments[0]["rect"].as_array().unwrap().len(), 4);
    let points = &fragments[0]["strokes"][0];
    assert_eq!(points[0].as_array().unwrap().len(), 3, "x, y and t");
    assert!(index.exists());
}

#[test]
fn ink_search_finds_read_writing_and_typed_text() {
    let dir = tempfile::tempdir().unwrap();
    let index = dir.path().join("a.json");
    let mut document = plain_document();
    draw_line(&mut document, 200.0);
    document.paste_text("Una matriz", Vector2::new(100.0, 400.0));
    let pending: serde_json::Value =
        serde_json::from_str(&document.ink_pending_json(&index, "es").unwrap()).unwrap();
    let hash = pending[0]["hash"].as_str().unwrap();
    crate::ink_index::store::add_reading(&index, "es", hash, [0.0; 4], &["Matriz".to_owned()])
        .unwrap();

    let found: serde_json::Value =
        serde_json::from_str(&document.ink_search_json(&index, "MATRIZ").unwrap()).unwrap();

    let hits = found.as_array().unwrap();
    assert_eq!(hits.len(), 2, "{found}");
    // The writing (at y = 200) comes before the text box (at y = 400).
    let top = |hit: &serde_json::Value| hit["rects"][0][1].as_f64().unwrap();
    assert!(top(&hits[0]) < 300.0 && top(&hits[1]) >= 390.0, "{found}");
    let typed = hits[1]["rects"][0].as_array().unwrap();
    let left = typed[0].as_f64().unwrap();
    assert!(left > 100.0, "the match starts after 'Una ': {found}");
}

#[test]
fn ink_search_without_an_index_finds_typed_text_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut document = plain_document();
    document.paste_text("hola", Vector2::new(0.0, 0.0));

    let found = document.ink_search_json(&dir.path().join("none.json"), "hola").unwrap();

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&found).unwrap().as_array().unwrap().len(),
        1
    );
}

#[test]
fn highlighter_strokes_are_not_writing() {
    use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;

    let mut document = plain_document();
    draw_line(&mut document, 100.0);
    document.set_brush_style(BrushStyle::Marker);
    draw_line(&mut document, 200.0);

    let strokes = document.ink_strokes();

    assert_eq!(strokes.len(), 1, "only the solid stroke: {strokes:?}");
}
