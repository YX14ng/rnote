//! Grouping strokes into lines and fragments.

use crate::ink_index::fragments::{InkStroke, fragments};

/// A stroke drawn as a zigzag inside the box from (`x`, `y`), `w` wide and `h` high.
fn letters(x: f64, y: f64, w: f64, h: f64) -> InkStroke {
    let points = (0..=8)
        .map(|i| {
            let fx = x + w * f64::from(i) / 8.0;
            let fy = if i % 2 == 0 { y + h } else { y };
            (fx, fy)
        })
        .collect();
    InkStroke { points }
}

/// A small mark, like the dot of an i.
fn dot(x: f64, y: f64) -> InkStroke {
    InkStroke { points: vec![(x, y), (x + 1.0, y + 1.0)] }
}

#[test]
fn two_words_on_a_line_are_two_fragments() {
    let strokes = [letters(0.0, 0.0, 60.0, 20.0), letters(100.0, 0.0, 60.0, 20.0)];

    let found = fragments(&strokes);

    assert_eq!(found.len(), 2);
    assert_eq!((found[0].line, found[1].line), (0, 0));
    assert!(found[0].rect[2] <= 60.0 + 1e-9 && found[1].rect[0] >= 100.0 - 1e-9);
}

#[test]
fn letters_close_together_are_one_fragment() {
    let strokes = [letters(0.0, 0.0, 30.0, 20.0), letters(34.0, 0.0, 30.0, 20.0)];

    assert_eq!(fragments(&strokes).len(), 1);
}

#[test]
fn two_lines_are_two_lines() {
    let strokes = [letters(0.0, 60.0, 60.0, 20.0), letters(0.0, 0.0, 60.0, 20.0)];

    let found = fragments(&strokes);

    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 0);
    assert_eq!(found[1].line, 1);
    assert!(found[0].rect[1] < found[1].rect[1], "the upper line comes first");
}

#[test]
fn a_dotted_i_stays_in_its_word() {
    let strokes = [letters(0.0, 0.0, 60.0, 20.0), dot(30.0, -8.0)];

    let found = fragments(&strokes);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].strokes.len(), 2);
    assert!(found[0].rect[1] <= -8.0);
}

#[test]
fn times_are_synthetic_and_increasing() {
    let strokes = [letters(0.0, 0.0, 30.0, 20.0), letters(34.0, 0.0, 30.0, 20.0)];

    let found = fragments(&strokes);

    let first = &found[0].strokes[0];
    let second = &found[0].strokes[1];
    assert_eq!(first[0].2, 0);
    assert_eq!(first[1].2, 10);
    assert_eq!(second[0].2, first.last().unwrap().2 + 100);
}

#[test]
fn hash_ignores_jitter_below_a_hundredth() {
    let base = [letters(0.0, 0.0, 60.0, 20.0)];
    let jittered = [letters(0.001, 0.001, 60.0, 20.0)];
    let moved = [letters(5.0, 0.0, 60.0, 20.0)];

    let hash = |strokes: &[InkStroke]| fragments(strokes)[0].hash.clone();

    assert_eq!(hash(&base), hash(&jittered));
    assert_ne!(hash(&base), hash(&moved));
    assert_eq!(hash(&base).len(), 64);
}

#[test]
fn no_strokes_no_fragments() {
    assert!(fragments(&[]).is_empty());
}

#[test]
fn close_lines_with_ascenders_and_descenders_stay_apart() {
    // x-height 20, ascenders and descenders 15, lines 54 apart: ruled paper.
    let strokes = [
        letters(0.0, 0.0, 60.0, 35.0),    // "agua", with a descender
        letters(100.0, 0.0, 60.0, 20.0),  // "casa"
        letters(0.0, 39.0, 60.0, 35.0),   // "hola", with an ascender
        letters(100.0, 54.0, 60.0, 20.0), // "mesa"
    ];

    let found = fragments(&strokes);

    assert_eq!(found.len(), 4, "{found:#?}");
    let lines: Vec<usize> = found.iter().map(|f| f.line).collect();
    assert_eq!(lines, [0, 0, 1, 1]);
}

#[test]
fn a_tall_stroke_doesnt_join_lines() {
    let strokes = [
        letters(0.0, 0.0, 60.0, 20.0),
        letters(0.0, 54.0, 60.0, 20.0),
        letters(-20.0, -5.0, 4.0, 85.0), // a bracket beside both lines
    ];

    let found = fragments(&strokes);

    let words: Vec<&_> = found.iter().filter(|f| f.rect[0] >= 0.0).collect();
    assert_eq!(words.len(), 2, "{found:#?}");
    assert_ne!(words[0].line, words[1].line);
}

#[test]
fn points_that_are_not_numbers_are_ignored() {
    let mut stroke = letters(0.0, 0.0, 60.0, 20.0);
    stroke.points.push((f64::NAN, 5.0));
    stroke.points.push((f64::INFINITY, 5.0));
    let strokes = [stroke, InkStroke { points: vec![(f64::NAN, f64::NAN)] }];

    let found = fragments(&strokes);

    assert_eq!(found.len(), 1);
    assert!(found[0].rect.iter().all(|v| v.is_finite()));
    assert!(found[0].strokes[0].iter().all(|&(x, y, _)| x.is_finite() && y.is_finite()));
}

#[test]
fn printed_letters_with_ascenders_and_descenders_stay_in_their_words() {
    // "lineal gana", a stroke per letter: x-height 20 on the baseline y = 120, the l's
    // reaching 28 above it and the g's tail 24 below.
    let strokes = [
        letters(0.0, 72.0, 4.0, 48.0),  // l
        letters(8.0, 100.0, 4.0, 20.0), // i
        dot(9.0, 92.0),
        letters(16.0, 100.0, 12.0, 20.0),  // n
        letters(32.0, 100.0, 12.0, 20.0),  // e
        letters(48.0, 100.0, 12.0, 20.0),  // a
        letters(64.0, 72.0, 4.0, 48.0),    // l
        letters(100.0, 100.0, 12.0, 20.0), // g's bowl
        letters(110.0, 100.0, 4.0, 44.0),  // g's tail
        letters(118.0, 100.0, 12.0, 20.0), // a
        letters(134.0, 100.0, 12.0, 20.0), // n
        letters(150.0, 100.0, 12.0, 20.0), // a
    ];

    let found = fragments(&strokes);

    assert_eq!(found.len(), 2, "{found:#?}");
    assert_eq!((found[0].line, found[1].line), (0, 0));
    assert_eq!((found[0].strokes.len(), found[1].strokes.len()), (7, 5));
}
