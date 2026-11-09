//! Formatting the selected part of the text being edited.

use rnote_compose::Color;
use rnote_engine::strokes::textstroke::{FontStyle, TextAttribute};

use super::{EditState, RnoteDocument};

/// Font weight of bold text (CSS scale), as the desktop app toggles it.
const BOLD_WEIGHT: u16 = 700;

/// Emphasis that can be toggled on part of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEmphasis {
    /// Bold weight.
    Bold,
    /// Italic style.
    Italic,
    /// Underlined.
    Underline,
    /// Struck through.
    Strikethrough,
}

impl RnoteDocument {
    /// Toggles `emphasis` on the selected part of the text being edited. Without a text
    /// selection, nothing changes.
    pub fn toggle_text_emphasis(&mut self, emphasis: TextEmphasis) -> EditState {
        let attribute = match emphasis {
            TextEmphasis::Bold => TextAttribute::FontWeight(BOLD_WEIGHT),
            TextEmphasis::Italic => TextAttribute::Style(FontStyle::Italic),
            TextEmphasis::Underline => TextAttribute::Underline(true),
            TextEmphasis::Strikethrough => TextAttribute::Strikethrough(true),
        };
        let bounds_before = self.bounds();
        let flags = self.engine.text_selection_toggle_attribute(attribute);
        self.finish_edit(flags, bounds_before)
    }

    /// Colors the selected part of the text being edited. Without a text selection, nothing
    /// changes.
    pub fn color_selected_text(&mut self, color: Color) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.text_selection_add_attribute(TextAttribute::TextColor(color));
        self.finish_edit(flags, bounds_before)
    }
}
