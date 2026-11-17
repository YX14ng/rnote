//! Searching the recognized writing and the typed text, without telling apart upper and
//! lower case or accented letters.

use std::collections::BTreeMap;
use std::ops::Range;

use unicode_normalization::UnicodeNormalization;

use super::fragments::Fragment;
use super::store::InkIndex;

/// A text box of the document.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TypedText {
    pub text: String,
    /// Its bounds, for the reading order: left, top, right, bottom.
    pub bounds: [f64; 4],
}

/// Where the query was found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Hit {
    /// In the writing: the fragments it covers.
    Ink { rects: Vec<[f64; 4]> },
    /// In the text box `text` of the ones searched, at the bytes `range` of its text.
    Typed { text: usize, range: Range<usize>, bounds: [f64; 4] },
}

/// Whether `c` is an accent laid over a letter (á, ü, ё, ç…): the Combining Diacritical
/// Marks block. Other marks make other letters (the voiced marks of Japanese kana) and stay.
fn is_accent(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c)
}

/// `text` in lower case, without accents, with single spaces and none at the ends.
pub(crate) fn normalize(text: &str) -> String {
    normalize_mapped(text).0
}

/// [`normalize`], with the bytes of `text` each byte of the result comes from.
fn normalize_mapped(text: &str) -> (String, Vec<Range<usize>>) {
    let mut out = String::new();
    let mut origin = Vec::new();
    let mut space: Option<Range<usize>> = None;
    for (start, c) in text.char_indices() {
        let range = start..start + c.len_utf8();
        if c.is_whitespace() {
            if !out.is_empty() && space.is_none() {
                space = Some(range);
            }
            continue;
        }
        let mut pieces = std::iter::once(c)
            .nfd()
            .filter(|piece| !is_accent(*piece))
            .flat_map(char::to_lowercase)
            .peekable();
        if pieces.peek().is_none() {
            // A mark written apart (an accent after its letter) belongs to the letter.
            if space.is_none()
                && let Some(last) = origin.last().map(|r: &Range<usize>| r.start)
            {
                for r in origin.iter_mut().rev().take_while(|r| r.start == last) {
                    r.end = range.end;
                }
            }
            continue;
        }
        if let Some(space) = space.take() {
            out.push(' ');
            origin.push(space);
        }
        for piece in pieces {
            out.push(piece);
            origin.extend(std::iter::repeat_n(range.clone(), piece.len_utf8()));
        }
    }
    (out, origin)
}

/// Every place `query` is in the writing read into `index` (for the `fragments` of the
/// document now) and in `typed`, in reading order.
pub(crate) fn search(
    index: &InkIndex,
    fragments: &[Fragment],
    typed: &[TypedText],
    query: &str,
) -> Vec<Hit> {
    let query = normalize(query);
    if query.is_empty() {
        return Vec::new();
    }
    // Hits in the writing sort by the top of their line, so a tall capital letter doesn't
    // put a word before the one on its left.
    let mut line_tops: BTreeMap<usize, f64> = BTreeMap::new();
    for fragment in fragments {
        let top = line_tops.entry(fragment.line).or_insert(fragment.rect[1]);
        *top = top.min(fragment.rect[1]);
    }
    let ink_key = |first: &Fragment| (line_tops[&first.line], first.rect[0]);
    let mut hits: Vec<((f64, f64), Hit)> = Vec::new();
    for fragment in fragments {
        let read = index.entries.get(&fragment.hash);
        if read.is_some_and(|entry| entry.candidates.iter().any(|c| normalize(c).contains(&query)))
        {
            hits.push((ink_key(fragment), Hit::Ink { rects: vec![fragment.rect] }));
        }
    }
    for covered in across_fragments(index, fragments, &query) {
        let rects = covered.iter().map(|&n| fragments[n].rect).collect();
        hits.push((ink_key(&fragments[covered[0]]), Hit::Ink { rects }));
    }
    for (number, text) in typed.iter().enumerate() {
        let (normalized, origin) = normalize_mapped(&text.text);
        for (start, found) in normalized.match_indices(&query) {
            let end = start + found.len();
            let hit = Hit::Typed {
                text: number,
                range: origin[start].start..origin[end - 1].end,
                bounds: text.bounds,
            };
            hits.push(((text.bounds[1], text.bounds[0]), hit));
        }
    }
    hits.sort_by(|(a, _), (b, _)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    hits.into_iter().map(|(_, hit)| hit).collect()
}

/// The places `query` goes across several fragments of a line, read with their best
/// reading, as the positions of those fragments in `fragments`: a phrase across words, or
/// a word the grouping split in two (its pieces read without the space between them).
fn across_fragments(index: &InkIndex, fragments: &[Fragment], query: &str) -> Vec<Vec<usize>> {
    if !query.contains(' ') {
        return read_together(index, fragments, query, "");
    }
    // A phrase, also without its spaces: one of its words may be split in pieces.
    let mut hits = read_together(index, fragments, query, " ");
    for covered in read_together(index, fragments, &query.replace(' ', ""), "") {
        if !hits.contains(&covered) {
            hits.push(covered);
        }
    }
    hits
}

/// [`across_fragments`], with the readings of a line put together with `between` them.
fn read_together(
    index: &InkIndex,
    fragments: &[Fragment],
    query: &str,
    between: &str,
) -> Vec<Vec<usize>> {
    let mut hits = Vec::new();
    let mut start = 0;
    while start < fragments.len() {
        let line = fragments[start].line;
        let end = fragments[start..]
            .iter()
            .position(|f| f.line != line)
            .map_or(fragments.len(), |n| start + n);
        // The line's text, and the fragment each byte comes from (none for the spaces).
        let mut text = String::new();
        let mut owner: Vec<Option<usize>> = Vec::new();
        for (number, fragment) in fragments.iter().enumerate().take(end).skip(start) {
            if !text.is_empty() {
                text.push_str(between);
                owner.extend(std::iter::repeat_n(None, between.len()));
            }
            match index.entries.get(&fragment.hash).and_then(|entry| entry.candidates.first()) {
                Some(best) => {
                    let best = normalize(best);
                    owner.extend(std::iter::repeat_n(Some(number), best.len()));
                    text.push_str(&best);
                }
                // Not read yet: nothing can go across it.
                None => {
                    text.push('\u{0}');
                    owner.push(None);
                }
            }
        }
        for (at, found) in text.match_indices(query) {
            let span = &owner[at..at + found.len()];
            let mut covered: Vec<usize> = span.iter().flatten().copied().collect();
            covered.dedup();
            // Pieces of a split word: the end of one and the start of the next alone would
            // also be the bits of two words where they meet.
            let whole_piece = covered.iter().any(|&n| {
                owner.iter().filter(|o| **o == Some(n)).count()
                    == span.iter().filter(|o| **o == Some(n)).count()
            });
            if covered.len() > 1 && (!between.is_empty() || whole_piece) {
                hits.push(covered);
            }
        }
        start = end;
    }
    hits
}
