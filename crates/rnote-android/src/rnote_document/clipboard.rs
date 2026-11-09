//! Copying, cutting and pasting.

use anyhow::Context;
use parry2d_f64::bounding_volume::{Aabb, BoundingVolume};
use parry2d_f64::math::Vector2;
use rnote_compose::Transformable;
use rnote_compose::shapes::Shapeable;
use rnote_engine::engine::StrokeContent;
use rnote_engine::strokes::Stroke;
use rnote_engine::strokes::resize::ImageSizeOption;
use rnote_engine::{WidgetFlags, pens::PenStyle};

use super::{EditState, RnoteDocument};

/// MIME type of plain text, as the engine reports it for selected text.
const TEXT_MIME_TYPE: &str = "text/plain";

/// Something to put on the clipboard: data and its MIME type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEntry {
    /// MIME type of [`Self::data`].
    pub mime_type: String,
    /// The content.
    pub data: Vec<u8>,
}

impl RnoteDocument {
    /// What the current pen offers to copy: the selected strokes or the selected text.
    ///
    /// Returns the entry other documents can paste back: Rnote stroke content, or plain text.
    pub fn copy(&self) -> anyhow::Result<Option<ClipboardEntry>> {
        let (entries, _flags) = futures::executor::block_on(self.engine.fetch_clipboard_content())
            .context("The copy task was cancelled")??;
        Ok(pick_entry(entries))
    }

    /// Like [`Self::copy`], then removes what was copied.
    pub fn cut(&mut self) -> anyhow::Result<(Option<ClipboardEntry>, EditState)> {
        let bounds_before = self.bounds();
        let (entries, flags) = futures::executor::block_on(self.engine.cut_clipboard_content())
            .context("The cut task was cancelled")??;
        Ok((pick_entry(entries), self.finish_edit(flags, bounds_before)))
    }

    /// Pastes strokes copied from a document, centered at `center`, selected.
    pub fn paste_stroke_content(
        &mut self,
        json: &str,
        center: Vector2,
    ) -> anyhow::Result<EditState> {
        let content: StrokeContent =
            serde_json::from_str(json).context("The clipboard holds no valid stroke content")?;
        // The engine places the strokes' top-left corner at the given position.
        let strokes_bounds = content
            .strokes
            .iter()
            .map(|layered| layered.stroke.bounds())
            .fold(Aabb::new_invalid(), |acc, bounds| acc.merged(&bounds));
        let top_left = center - strokes_bounds.extents() / 2.0;
        let bounds_before = self.bounds();
        let mut flags = self.ensure_pen_mode();
        flags |= self.engine.insert_stroke_content(
            content,
            top_left,
            ImageSizeOption::RespectOriginalSize,
        );
        self.pen_style = PenStyle::Selector;
        // The engine doesn't flag insertions as store modifications.
        self.modified = true;
        Ok(self.finish_edit(flags, bounds_before))
    }

    /// Pastes text as a new text box starting at `pos`, ready to be edited.
    pub fn paste_text(&mut self, text: &str, pos: Vector2) -> EditState {
        let bounds_before = self.bounds();
        let mut flags = self.ensure_pen_mode();
        flags |= self.engine.insert_text(text.to_owned(), Some(pos));
        self.pen_style = PenStyle::Typewriter;
        self.modified = true;
        self.finish_edit(flags, bounds_before)
    }

    /// Pastes an image (PNG, JPEG, ...) centered at `center`, selected.
    pub fn paste_image(&mut self, bytes: Vec<u8>, center: Vector2) -> anyhow::Result<EditState> {
        let mut image = futures::executor::block_on(
            self.engine
                .generate_bitmapimage_from_bytes(center, bytes, /* respect_borders */ true),
        )
        .context("The image import task was cancelled")?
        .context("The clipboard image could not be read")?;
        image.translate(center - image.bounds().center());
        let bounds_before = self.bounds();
        let mut flags: WidgetFlags = self.ensure_pen_mode();
        flags |= self.engine.import_generated_content(
            vec![(Stroke::BitmapImage(image), None)],
            /* adjust_document */ false,
        );
        // Imported content ends up selected, with the selector.
        self.pen_style = PenStyle::Selector;
        self.modified = true;
        Ok(self.finish_edit(flags, bounds_before))
    }
}

/// Prefers Rnote stroke content, which pastes back losslessly, then plain text.
fn pick_entry(entries: Vec<(Vec<u8>, String)>) -> Option<ClipboardEntry> {
    let entry = |(data, mime_type): (Vec<u8>, String)| ClipboardEntry { mime_type, data };
    let mut text = None;
    for (data, mime_type) in entries {
        if mime_type == StrokeContent::MIME_TYPE {
            return Some(entry((data, mime_type)));
        }
        if mime_type == TEXT_MIME_TYPE {
            text = Some(entry((data, mime_type)));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_stroke_content_over_text_and_images() {
        let entries = vec![
            (b"png".to_vec(), "image/png".to_owned()),
            (b"text".to_vec(), TEXT_MIME_TYPE.to_owned()),
            (b"{}".to_vec(), StrokeContent::MIME_TYPE.to_owned()),
        ];

        let picked = pick_entry(entries).expect("an entry should be picked");

        assert_eq!(picked.mime_type, StrokeContent::MIME_TYPE);
    }

    #[test]
    fn falls_back_to_text_and_skips_images() {
        let only_text = vec![
            (b"png".to_vec(), "image/png".to_owned()),
            (b"hola".to_vec(), TEXT_MIME_TYPE.to_owned()),
        ];
        let only_image = vec![(b"png".to_vec(), "image/png".to_owned())];

        assert_eq!(pick_entry(only_text).map(|entry| entry.data), Some(b"hola".to_vec()));
        assert_eq!(pick_entry(only_image), None);
    }
}
