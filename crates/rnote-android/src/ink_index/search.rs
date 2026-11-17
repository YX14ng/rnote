//! Searching the recognized writing and the typed text, without telling apart upper and
//! lower case or accented letters.

use std::ops::Range;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

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

impl Hit {
    /// Where it starts, for the reading order: top, then left.
    fn position(&self) -> (f64, f64) {
        match self {
            Self::Ink { rects } => rects.first().map_or((0.0, 0.0), |r| (r[1], r[0])),
            Self::Typed { bounds, .. } => (bounds[1], bounds[0]),
        }
    }
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
            .filter(|piece| !is_combining_mark(*piece))
            .flat_map(char::to_lowercase)
            .peekable();
        if pieces.peek().is_none() {
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
    let mut hits = Vec::new();
    for fragment in fragments {
        let read = index.entries.get(&fragment.hash);
        if read.is_some_and(|entry| entry.candidates.iter().any(|c| normalize(c).contains(&query)))
        {
            hits.push(Hit::Ink { rects: vec![fragment.rect] });
        }
    }
    if query.contains(' ') {
        hits.extend(phrases(index, fragments, &query));
    }
    for (number, text) in typed.iter().enumerate() {
        let (normalized, origin) = normalize_mapped(&text.text);
        for (start, found) in normalized.match_indices(&query) {
            let end = start + found.len();
            hits.push(Hit::Typed {
                text: number,
                range: origin[start].start..origin[end - 1].end,
                bounds: text.bounds,
            });
        }
    }
    hits.sort_by(|a, b| {
        let (a, b) = (a.position(), b.position());
        a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
    });
    hits
}

/// The places a phrase spans several fragments of a line, read with their best reading.
fn phrases(index: &InkIndex, fragments: &[Fragment], query: &str) -> Vec<Hit> {
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
                text.push(' ');
                owner.push(None);
            }
            match index.entries.get(&fragment.hash).and_then(|entry| entry.candidates.first()) {
                Some(best) => {
                    let best = normalize(best);
                    owner.extend(std::iter::repeat_n(Some(number), best.len()));
                    text.push_str(&best);
                }
                // Not read yet: a phrase can't go across it.
                None => {
                    text.push('\u{0}');
                    owner.push(None);
                }
            }
        }
        for (at, found) in text.match_indices(query) {
            let mut covered: Vec<usize> =
                owner[at..at + found.len()].iter().flatten().copied().collect();
            covered.dedup();
            if covered.len() > 1 {
                hits.push(Hit::Ink { rects: covered.iter().map(|&n| fragments[n].rect).collect() });
            }
        }
        start = end;
    }
    hits
}
