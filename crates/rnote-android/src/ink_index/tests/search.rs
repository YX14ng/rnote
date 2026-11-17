//! Searching the index and the typed text.

use crate::ink_index::fragments::{InkStroke, fragments};
use crate::ink_index::search::{Hit, TypedText, normalize, search};
use crate::ink_index::store::{Entry, InkIndex};

/// A word written at (`x`, `y`), 40 wide and 20 high.
fn word(x: f64, y: f64) -> InkStroke {
    InkStroke { points: vec![(x, y), (x + 20.0, y + 20.0), (x + 40.0, y)] }
}

/// An index where the fragments of `strokes` read as `texts` (each its candidates).
fn indexed(
    strokes: &[InkStroke],
    texts: &[&[&str]],
) -> (InkIndex, Vec<crate::ink_index::fragments::Fragment>) {
    let found = fragments(strokes);
    let mut index = InkIndex::empty("es");
    for (fragment, candidates) in found.iter().zip(texts) {
        let candidates = candidates.iter().map(|c| (*c).to_owned()).collect();
        index.entries.insert(fragment.hash.clone(), Entry { rect: fragment.rect, candidates });
    }
    (index, found)
}

fn ink_rects(hits: &[Hit]) -> Vec<Vec<[f64; 4]>> {
    hits.iter()
        .map(|hit| match hit {
            Hit::Ink { rects } => rects.clone(),
            Hit::Typed { .. } => panic!("expected ink, got {hit:?}"),
        })
        .collect()
}

#[test]
fn normalize_drops_case_accents_and_extra_spaces() {
    assert_eq!(normalize("  Álgebra   LINEAL "), "algebra lineal");
    assert_eq!(normalize("Función"), "funcion");
    assert_eq!(normalize("Ñandú"), "nandu");
}

#[test]
fn search_ignores_case_and_accents() {
    let (index, found) =
        indexed(&[word(0.0, 0.0), word(100.0, 0.0)], &[&["álgebra"], &["Función"]]);

    assert_eq!(ink_rects(&search(&index, &found, &[], "ALGEBRA")), [vec![found[0].rect]]);
    assert_eq!(ink_rects(&search(&index, &found, &[], "funcion")), [vec![found[1].rect]]);
    assert_eq!(ink_rects(&search(&index, &found, &[], "unci")), [vec![found[1].rect]]);
}

#[test]
fn any_of_three_candidates_matches() {
    let (index, found) = indexed(&[word(0.0, 0.0)], &[&["matnz", "matriz", "motriz"]]);

    assert_eq!(search(&index, &found, &[], "matriz").len(), 1);
}

#[test]
fn phrase_spans_fragments_of_a_line() {
    let (index, found) = indexed(
        &[word(0.0, 0.0), word(100.0, 0.0), word(200.0, 0.0)],
        &[&["la"], &["matriz"], &["inversa"]],
    );

    let hits = ink_rects(&search(&index, &found, &[], "matriz inversa"));

    assert_eq!(hits, [vec![found[1].rect, found[2].rect]]);
}

#[test]
fn phrase_never_spans_lines() {
    let (index, found) = indexed(&[word(0.0, 0.0), word(0.0, 100.0)], &[&["matriz"], &["inversa"]]);

    assert!(search(&index, &found, &[], "matriz inversa").is_empty());
}

#[test]
fn unrecognized_fragments_are_skipped() {
    let found = fragments(&[word(0.0, 0.0)]);

    assert!(search(&InkIndex::empty("es"), &found, &[], "a").is_empty());
}

#[test]
fn typed_text_matches_with_its_byte_range() {
    let typed =
        [TypedText { text: "Una Matríz útil".to_owned(), bounds: [0.0, 0.0, 100.0, 20.0] }];

    let hits = search(&InkIndex::empty("es"), &[], &typed, "matriz");

    assert_eq!(hits.len(), 1);
    let Hit::Typed { text, range, .. } = &hits[0] else { panic!("{hits:?}") };
    assert_eq!(*text, 0);
    assert_eq!(&typed[0].text[range.clone()], "Matríz");
}

#[test]
fn every_occurrence_is_a_hit() {
    let typed = [TypedText { text: "ab ab ab".to_owned(), bounds: [0.0; 4] }];

    assert_eq!(search(&InkIndex::empty("es"), &[], &typed, "ab").len(), 3);
}

#[test]
fn results_are_in_reading_order() {
    let (index, found) =
        indexed(&[word(100.0, 100.0), word(0.0, 100.0), word(0.0, 0.0)], &[&["x"], &["x"], &["x"]]);
    let typed = [TypedText { text: "x".to_owned(), bounds: [500.0, 50.0, 520.0, 70.0] }];

    let hits = search(&index, &found, &typed, "x");

    let tops: Vec<(f64, f64)> = hits
        .iter()
        .map(|hit| match hit {
            Hit::Ink { rects } => (rects[0][1], rects[0][0]),
            Hit::Typed { bounds, .. } => (bounds[1], bounds[0]),
        })
        .collect();
    assert_eq!(tops, [(0.0, 0.0), (50.0, 500.0), (100.0, 0.0), (100.0, 100.0)]);
}

#[test]
fn an_empty_query_finds_nothing() {
    let typed = [TypedText { text: "x".to_owned(), bounds: [0.0; 4] }];

    assert!(search(&InkIndex::empty("es"), &[], &typed, "  ").is_empty());
}

#[test]
fn a_word_split_in_two_fragments_is_found() {
    let (index, found) = indexed(&[word(0.0, 0.0), word(100.0, 0.0)], &[&["alge"], &["bra"]]);

    let hits = ink_rects(&search(&index, &found, &[], "algebra"));

    assert_eq!(hits, [vec![found[0].rect, found[1].rect]]);
}

#[test]
fn a_split_word_doesnt_hide_the_words_found_whole() {
    let (index, found) = indexed(&[word(0.0, 0.0), word(100.0, 0.0)], &[&["matriz"], &["inversa"]]);

    assert_eq!(ink_rects(&search(&index, &found, &[], "matriz")), [vec![found[0].rect]]);
    // Bits of two words that only meet without the space between them aren't a word.
    assert!(search(&index, &found, &[], "zin").is_empty());
}

#[test]
fn part_of_a_split_word_is_found_when_it_covers_one_piece() {
    let (index, found) = indexed(&[word(0.0, 0.0), word(100.0, 0.0)], &[&["alge"], &["bra"]]);

    assert_eq!(search(&index, &found, &[], "gebra").len(), 1);
    assert_eq!(search(&index, &found, &[], "algeb").len(), 1);
}

#[test]
fn hits_on_a_line_go_left_to_right_even_with_a_tall_letter() {
    // "es" and then "Lineal", whose capital L starts above the "es".
    let short = InkStroke { points: vec![(0.0, 10.0), (20.0, 20.0), (40.0, 10.0)] };
    let tall = InkStroke { points: vec![(100.0, 0.0), (120.0, 20.0), (140.0, 0.0)] };
    let (index, found) = indexed(&[short, tall], &[&["esa"], &["Lineal"]]);
    let left = found.iter().find(|f| f.rect[0] == 0.0).unwrap().rect;
    let right = found.iter().find(|f| f.rect[0] == 100.0).unwrap().rect;

    let hits = ink_rects(&search(&index, &found, &[], "a"));

    assert_eq!(hits, [vec![left], vec![right]]);
}

#[test]
fn a_typed_accent_written_apart_is_part_of_its_letter() {
    // "café" with the accent as its own character after the e.
    let typed = [TypedText { text: "cafe\u{301} con leche".to_owned(), bounds: [0.0; 4] }];

    let hits = search(&InkIndex::empty("es"), &[], &typed, "cafe");

    assert!(matches!(&hits[..], [Hit::Typed { range, .. }] if *range == (0..6)), "{hits:?}");
}
