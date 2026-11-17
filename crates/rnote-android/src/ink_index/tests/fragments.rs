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
