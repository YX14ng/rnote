//! The writing and the text of the document, for the ink index and the search.

use std::path::Path;

use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_compose::ext::DAffine2Ext;
use rnote_compose::shapes::Shapeable;
use rnote_engine::strokes::{Stroke, TextStroke};
use serde_json::json;
use unicode_segmentation::GraphemeCursor;

use super::RnoteDocument;
use crate::ink_index::fragments::{self, InkStroke};
use crate::ink_index::search::{self, Hit, TypedText};
use crate::ink_index::store::{self, InkIndex};

// Only the JNI entry points, built for Android, call them outside of tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
impl RnoteDocument {
    /// The pen strokes, in the order they were drawn.
    pub(crate) fn ink_strokes(&self) -> Vec<InkStroke> {
        fragments::ink_strokes(&self.engine)
    }

    /// The fragments of the writing the index at `index_path` (in `language`) has no
    /// reading of, as JSON: `[{"hash", "rect": [4], "strokes": [[[x, y, t], ...], ...]}]`.
    /// Readings of fragments that are gone are dropped from the index.
    pub(crate) fn ink_pending_json(
        &self,
        index_path: &Path,
        language: &str,
    ) -> anyhow::Result<String> {
        let current = fragments::fragments(&self.ink_strokes());
        let todo =
            store::with_index(index_path, language, |index| store::pending(index, &current))?;
        let list: Vec<_> = todo
            .iter()
            .map(|fragment| {
                let strokes: Vec<Vec<[f64; 3]>> = fragment
                    .strokes
                    .iter()
                    .map(|stroke| stroke.iter().map(|&(x, y, t)| [x, y, t as f64]).collect())
                    .collect();
                json!({ "hash": fragment.hash, "rect": fragment.rect, "strokes": strokes })
            })
            .collect();
        Ok(serde_json::to_string(&list)?)
    }

    /// Where `query` is in the writing read into the index at `index_path` and in the text
    /// boxes, in reading order, as JSON: `[{"rects": [[left, top, right, bottom], ...]}]`.
    pub(crate) fn ink_search_json(&self, index_path: &Path, query: &str) -> anyhow::Result<String> {
        let current = fragments::fragments(&self.ink_strokes());
        let index = InkIndex::stored_language(index_path)
            .map_or_else(|| InkIndex::empty(""), |language| InkIndex::load(index_path, &language));
        let texts = self.text_strokes();
        let typed: Vec<TypedText> = texts
            .iter()
            .map(|(text, bounds)| TypedText { text: text.text.clone(), bounds: *bounds })
            .collect();
        let list: Vec<_> = search::search(&index, &current, &typed, query)
            .into_iter()
            .map(|hit| {
                let rects = match hit {
                    Hit::Ink { rects } => rects,
                    Hit::Typed { text, range, bounds } => {
                        typed_rects(&texts[text].0, range).unwrap_or_else(|| vec![bounds])
                    }
                };
                json!({ "rects": rects })
            })
            .collect();
        Ok(serde_json::to_string(&list)?)
    }

    /// The text boxes with their bounds.
    fn text_strokes(&self) -> Vec<(TextStroke, [f64; 4])> {
        let everything = Aabb::new(Vector2::splat(-1e9), Vector2::splat(1e9));
        self.engine
            .extract_content_in_bounds(everything)
            .strokes
            .iter()
            .filter_map(|layered| match layered.stroke.as_ref() {
                Stroke::TextStroke(text) => {
                    let b = layered.stroke.bounds();
                    Some((text.clone(), [b.mins.x, b.mins.y, b.maxs.x, b.maxs.y]))
                }
                _ => None,
            })
            .collect()
    }
}

/// Where the bytes `range` of `text` are on the page, a rectangle per line they cover.
fn typed_rects(text: &TextStroke, range: std::ops::Range<usize>) -> Option<Vec<[f64; 4]>> {
    let len = text.text.len();
    let start = GraphemeCursor::new(range.start, len, true);
    let end = GraphemeCursor::new(range.end, len, true);
    let rects =
        text.text_style.get_selection_rects_for_cursors(text.text.clone(), &start, &end).ok()?;
    let affine = text.affine.to_kurbo();
    let rects: Vec<[f64; 4]> = rects
        .into_iter()
        .map(|rect| {
            let r = affine.transform_rect_bbox(rect);
            [r.x0, r.y0, r.x1, r.y1]
        })
        .collect();
    (!rects.is_empty()).then_some(rects)
}
