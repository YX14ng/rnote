//! Putting the view back where it was after the document under it was replaced, e.g. by a
//! newer version of the same notebook from another device.

use p2d::math::Vector2;

use crate::{Engine, WidgetFlags};

impl Engine {
    /// Shows the document at `offset` with `zoom` on a view of `size` (this screen's: the
    /// document may come with another device's), and draws everything there again, as a zoom
    /// does: what was drawn belonged to the view the document came with.
    pub fn restore_view(&mut self, offset: Vector2, zoom: f64, size: Vector2) -> WidgetFlags {
        let mut widget_flags = self.camera.set_size(size);
        widget_flags |= self.camera.zoom_temporarily_to(1.0) | self.camera.zoom_to(zoom);
        // The document first takes its size, which bounds where the view may go.
        widget_flags |= self.doc_resize_autoexpand();
        widget_flags |= self.camera_set_offset_expand(offset);
        let all_strokes = self.store.stroke_keys_unordered();
        self.store.set_rendering_dirty_for_strokes(&all_strokes);
        widget_flags
            | self.background_rendering_regenerate()
            | self.update_rendering_current_viewport()
    }
}

#[cfg(test)]
mod tests;
