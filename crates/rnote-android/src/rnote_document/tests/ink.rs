//! The writing of a document, as the ink index reads it.

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
