//! Working with the pages of a fixed-size document as a whole: deleting, moving or
//! duplicating some of them and inserting blank pages or imported pages between the others.
//!
//! A stroke belongs to the page its center is on. Removing or inserting pages moves the
//! strokes of the pages below, so the content stays on its page.

use crate::document::Layout;
use crate::engine::StrokeContent;
use crate::engine::export::DocExportPrefs;
use crate::store::StrokeKey;
use crate::store::chrono_comp::StrokeLayer;
use crate::strokes::Stroke;
use crate::{Engine, WidgetFlags};
use p2d::bounding_volume::Aabb;
use p2d::math::Vector2;
use rnote_compose::Transformable;
use rnote_compose::shapes::Shapeable;
use std::time::Instant;

impl Engine {
    /// The pages a document export with `prefs` has: the ones with something on them, or
    /// every page when blank pages are exported too.
    pub fn export_pages_content(&self, prefs: &DocExportPrefs) -> Vec<StrokeContent> {
        if prefs.with_empty_pages {
            self.extract_all_pages_content(prefs.page_order)
        } else {
            self.extract_pages_content(prefs.page_order)
        }
    }

    /// The number of pages, when in fixed-size layout.
    pub fn doc_page_count_fixed_size(&self) -> Option<u32> {
        let height = self.document.config.format.height();
        if self.document.config.layout != Layout::FixedSize || height <= 0.0 {
            return None;
        }
        Some(((self.document.height / height).round() as u32).max(1))
    }

    /// The page (0-based) at the height `y`, when in fixed-size layout; heights above or
    /// below the document give the first or the last page.
    pub fn doc_page_at_fixed_size(&self, y: f64) -> Option<u32> {
        let count = self.doc_page_count_fixed_size()?;
        let height = self.document.config.format.height();
        let page = ((y - self.document.y) / height).floor().max(0.0) as u32;
        Some(page.min(count - 1))
    }

    /// The height of the top of `page` (0-based), when in fixed-size layout.
    pub fn doc_page_top_fixed_size(&self, page: u32) -> Option<f64> {
        let count = self.doc_page_count_fixed_size()?;
        let height = self.document.config.format.height();
        (page < count).then(|| self.document.y + f64::from(page) * height)
    }

    /// Whether each page has strokes on it, when in fixed-size layout.
    pub fn doc_pages_with_content_fixed_size(&self) -> Vec<bool> {
        let Some(count) = self.doc_page_count_fixed_size() else {
            return vec![];
        };
        let mut with_content = vec![false; count as usize];
        for key in self.store.stroke_keys_as_rendered() {
            if let Some(page) = self.page_of_stroke(key)
                && let Some(slot) = with_content.get_mut(page as usize)
            {
                *slot = true;
            }
        }
        with_content
    }

    /// Removes the `pages` (0-based) and what is on them; the pages below move up. At least
    /// one page stays: removing every page leaves a single blank one.
    ///
    /// Recorded as one change. Does nothing outside fixed-size layout.
    pub fn doc_remove_pages_fixed_size(&mut self, pages: &[u32]) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();
        let Some(count) = self.doc_page_count_fixed_size() else {
            return widget_flags;
        };
        let mut removed: Vec<u32> = pages.iter().copied().filter(|&p| p < count).collect();
        removed.sort_unstable();
        removed.dedup();
        if removed.is_empty() {
            return widget_flags;
        }
        let height = self.document.config.format.height();
        let mut trashed = Vec::new();
        for key in self.store.stroke_keys_as_rendered() {
            let Some(page) = self.page_of_stroke(key) else {
                continue;
            };
            if removed.binary_search(&page).is_ok() {
                trashed.push(key);
                continue;
            }
            let above = removed.partition_point(|&p| p < page);
            if above > 0 {
                self.move_strokes(&[key], -(above as f64) * height);
            }
        }
        self.store.set_selected_keys(&trashed, false);
        self.store.set_trashed_keys(&trashed, true);
        let left = count.saturating_sub(removed.len() as u32).max(1);
        self.document.height = f64::from(left) * height;
        widget_flags |= self.finish_pages_change();
        widget_flags
    }

    /// Inserts `count` blank pages before page `at` (0-based; the page count appends them).
    ///
    /// Recorded as one change. Does nothing outside fixed-size layout.
    pub fn doc_insert_pages_fixed_size(&mut self, at: u32, count: u32) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();
        if count == 0 || self.make_room_for_pages(at, count).is_none() {
            return widget_flags;
        }
        widget_flags |= self.finish_pages_change();
        widget_flags
    }

    /// Inserts `pages`, e.g. the pages of an imported PDF, before page `at` (0-based): each
    /// on pages of its own, as many as it is tall, at their top left. They go on the
    /// document layer, under the ink, and stay unselected.
    ///
    /// Recorded as one change. Does nothing outside fixed-size layout.
    pub fn doc_insert_page_strokes_fixed_size(
        &mut self,
        at: u32,
        pages: Vec<Stroke>,
    ) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();
        let height = self.document.config.format.height();
        if pages.is_empty() || height <= 0.0 {
            return widget_flags;
        }
        let spans: Vec<u32> = pages
            .iter()
            .map(|page| Self::sheets_for(page.bounds().extents()[1], height))
            .collect();
        let Some(at) = self.make_room_for_pages(at, spans.iter().sum()) else {
            return widget_flags;
        };
        let mut next = at;
        for (mut page, span) in pages.into_iter().zip(spans) {
            let bounds = page.bounds();
            let top_left =
                Vector2::new(self.document.x, self.document.y + f64::from(next) * height);
            page.translate(top_left - bounds.mins);
            self.store.insert_stroke(page, Some(StrokeLayer::Document));
            next += span;
        }
        widget_flags |= self.finish_pages_change();
        widget_flags
    }

    /// Moves page `from` to be page `to` (both 0-based); the pages between move one place
    /// to make room, as in a list. Recorded as one change. Does nothing outside fixed-size
    /// layout or for pages past the end.
    pub fn doc_move_page_fixed_size(&mut self, from: u32, to: u32) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();
        let Some(count) = self.doc_page_count_fixed_size() else {
            return widget_flags;
        };
        if from == to || from >= count || to >= count {
            return widget_flags;
        }
        let height = self.document.config.format.height();
        for key in self.store.stroke_keys_as_rendered() {
            let Some(page) = self.page_of_stroke(key) else {
                continue;
            };
            let new_page = if page == from {
                to
            } else if from < to && (from + 1..=to).contains(&page) {
                page - 1
            } else if to < from && (to..from).contains(&page) {
                page + 1
            } else {
                continue;
            };
            self.move_strokes(&[key], (f64::from(new_page) - f64::from(page)) * height);
        }
        widget_flags |= self.finish_pages_change();
        widget_flags
    }

    /// Puts a copy of page `page` (0-based), with everything on it, on a new page right
    /// after it. Recorded as one change. Does nothing outside fixed-size layout or for a
    /// page past the end.
    pub fn doc_duplicate_page_fixed_size(&mut self, page: u32) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();
        if self
            .doc_page_count_fixed_size()
            .is_none_or(|count| page >= count)
        {
            return widget_flags;
        }
        let height = self.document.config.format.height();
        let top = self.document.y + f64::from(page) * height;
        let sheet = Aabb::new(
            Vector2::new(self.document.x, top),
            Vector2::new(self.document.x + self.document.width, top + height),
        );
        // What is on the page, by the same rule as `page_of_stroke`: its center. Each copy
        // keeps its layer, so a PDF page stays under the ink.
        let copies: Vec<(Stroke, StrokeLayer)> = self
            .extract_content_in_bounds(sheet)
            .strokes
            .into_iter()
            .filter(|layered| {
                let center = layered.stroke.bounds().center()[1];
                center >= top && center < top + height
            })
            .map(|layered| ((*layered.stroke).clone(), layered.layer))
            .collect();
        if self.make_room_for_pages(page + 1, 1).is_none() {
            return widget_flags;
        }
        for (mut stroke, layer) in copies {
            stroke.translate(Vector2::new(0.0, height));
            self.store.insert_stroke(stroke, Some(layer));
        }
        widget_flags |= self.finish_pages_change();
        widget_flags
    }

    /// Grows a fixed-size document to hold all of its strokes, never shrinking it: undoing
    /// a change to the pages brings back strokes, but not the pages they were on.
    pub fn doc_expand_fixed_size_to_content(&mut self) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();
        let height = self.document.config.format.height();
        if self.document.config.layout != Layout::FixedSize || height <= 0.0 {
            return widget_flags;
        }
        let content_bottom = self
            .store
            .strokes_bounds(&self.store.stroke_keys_as_rendered())
            .into_iter()
            .map(|bounds| bounds.maxs[1])
            .fold(self.document.y, f64::max);
        let needed = ((content_bottom - self.document.y) / height).ceil() * height;
        if needed > self.document.height + 0.5 {
            self.document.height = needed;
            widget_flags |= self.update_rendering_current_viewport();
            widget_flags.resize = true;
        }
        widget_flags
    }

    /// How many sheets of `sheet_height` a page `height` tall takes. A page barely taller than
    /// a sheet, like an A4 PDF page on an A4 sheet rounded differently, takes one.
    fn sheets_for(height: f64, sheet_height: f64) -> u32 {
        const SLACK: f64 = 0.02;
        ((height / sheet_height - SLACK).ceil() as u32).max(1)
    }

    /// The page the center of the stroke is on, if it is below the top of the document.
    fn page_of_stroke(&self, key: StrokeKey) -> Option<u32> {
        let height = self.document.config.format.height();
        let stroke = self.store.get_stroke_ref(key)?;
        let center = stroke.bounds().center()[1] - self.document.y;
        (center >= 0.0 && height > 0.0).then(|| (center / height).floor() as u32)
    }

    /// Moves the strokes of the pages from `at` on down by `count` pages and grows the
    /// document by as much. Returns where the new pages start, clamped to the page count.
    fn make_room_for_pages(&mut self, at: u32, count: u32) -> Option<u32> {
        let pages = self.doc_page_count_fixed_size()?;
        let at = at.min(pages);
        let height = self.document.config.format.height();
        let below: Vec<StrokeKey> = self
            .store
            .stroke_keys_as_rendered()
            .into_iter()
            .filter(|&key| self.page_of_stroke(key).is_some_and(|page| page >= at))
            .collect();
        self.move_strokes(&below, f64::from(count) * height);
        self.document.height = f64::from(pages + count) * height;
        Some(at)
    }

    fn move_strokes(&mut self, keys: &[StrokeKey], dy: f64) {
        let offset = Vector2::new(0.0, dy);
        self.store.translate_strokes(keys, offset);
        self.store.translate_strokes_images(keys, offset);
    }

    fn finish_pages_change(&mut self) -> WidgetFlags {
        let mut widget_flags = self.store.record(Instant::now());
        widget_flags |= self.current_pen_update_state();
        widget_flags |= self.update_rendering_current_viewport();
        widget_flags.resize = true;
        widget_flags.store_modified = true;
        widget_flags.refresh_ui = true;
        widget_flags
    }
}
