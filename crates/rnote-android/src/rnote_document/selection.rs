//! What the selection bar needs: where the selection is, and recoloring it.

use parry2d_f64::bounding_volume::{Aabb, BoundingVolume};
use rnote_compose::Color;
use rnote_compose::shapes::Shapeable;

use super::{EditState, RnoteDocument};

impl RnoteDocument {
    /// The bounds of the selected strokes, in document coordinates, if any are selected.
    #[must_use]
    pub fn selection_bounds(&self) -> Option<Aabb> {
        let content = self.engine.extract_selection_content()?;
        content
            .strokes
            .iter()
            .map(|layered| layered.stroke.bounds())
            .reduce(|acc, bounds| acc.merged(&bounds))
    }

    /// Gives the selected strokes `color`, as one undo step.
    pub fn recolor_selection(&mut self, color: Color) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.change_selection_stroke_colors(color);
        self.finish_edit(flags, bounds_before)
    }
}
