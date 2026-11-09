//! Pen input, keys, tools, selection and history.

use std::collections::HashSet;
use std::time::Instant;

use rnote_compose::PenEvent;
use rnote_compose::penevent::{KeyboardKey, ModifierKey};
use rnote_engine::WidgetFlags;
use rnote_engine::pens::{PenMode, PenStyle};

use super::{EditState, RnoteDocument, ViewSize};
use crate::pen_input::{self, PenPhase, PenSample, Tool};
use crate::render::Viewport;

impl RnoteDocument {
    /// Switches the tool used for the pen side of the stylus.
    ///
    /// Leaving the selector finishes the selection, like in the desktop app.
    pub fn set_tool(&mut self, tool: Tool) -> EditState {
        let bounds_before = self.bounds();
        let flags = match tool {
            Tool::Brush => self.use_pen_style(PenStyle::Brush),
            Tool::Shaper => self.use_pen_style(PenStyle::Shaper),
            Tool::Selector => self.use_pen_style(PenStyle::Selector),
            Tool::Typewriter => self.use_pen_style(PenStyle::Typewriter),
            // The eraser is a pen mode, not a style: the pen side keeps its tool.
            Tool::Eraser => WidgetFlags::default(),
        };
        self.finish_edit(flags, bounds_before)
    }

    /// Selects every stroke, switching to the selector.
    pub fn select_all(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.ensure_pen_mode() | self.engine.select_all_strokes();
        self.pen_style = PenStyle::Selector;
        self.finish_edit(flags, bounds_before)
    }

    /// Clears the selection.
    pub fn deselect_all(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.ensure_pen_mode() | self.engine.deselect_all_strokes();
        self.pen_style = PenStyle::Selector;
        self.finish_edit(flags, bounds_before)
    }

    /// Deletes the selected strokes.
    pub fn delete_selection(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.trash_selection();
        self.finish_edit(flags, bounds_before)
    }

    /// Duplicates the selected strokes; the copies become the selection.
    pub fn duplicate_selection(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.duplicate_selection();
        self.finish_edit(flags, bounds_before)
    }

    /// Feeds a batch of stylus samples to the engine.
    ///
    /// `viewport` and `view_size` describe what the user sees, which some pens depend on.
    pub fn handle_pen(
        &mut self,
        phase: PenPhase,
        tool: Tool,
        samples: &[PenSample],
        viewport: Viewport,
        view_size: ViewSize,
    ) -> EditState {
        let bounds_before = self.bounds();
        let mut flags = self.sync_camera(viewport, view_size);
        let pen_mode = match tool {
            Tool::Brush => {
                flags |= self.use_pen_style(PenStyle::Brush);
                PenMode::Pen
            }
            Tool::Shaper => {
                flags |= self.use_pen_style(PenStyle::Shaper);
                PenMode::Pen
            }
            Tool::Selector => {
                flags |= self.use_pen_style(PenStyle::Selector);
                PenMode::Pen
            }
            Tool::Typewriter => {
                flags |= self.use_pen_style(PenStyle::Typewriter);
                PenMode::Pen
            }
            Tool::Eraser => PenMode::Eraser,
        };
        let follows = if tool == Tool::Brush {
            self.follow_brush_stroke(phase, samples)
        } else {
            self.reset_recognition();
            true
        };
        if follows {
            let now = Instant::now();
            for event in pen_input::pen_events(phase, samples) {
                let (_propagation, event_flags) =
                    self.engine.handle_pen_event(event, Some(pen_mode), now);
                flags |= event_flags;
            }
        }
        self.finish_edit(flags, bounds_before)
    }

    /// Feeds a key press to the current pen, e.g. editing keys to the text tool, or Enter to
    /// finish a polyline or polygon.
    pub fn handle_key(&mut self, key: KeyboardKey, shift: bool, ctrl: bool) -> EditState {
        let mut modifier_keys = HashSet::new();
        if shift {
            modifier_keys.insert(ModifierKey::KeyboardShift);
        }
        if ctrl {
            modifier_keys.insert(ModifierKey::KeyboardCtrl);
        }
        self.handle_keyboard_event(PenEvent::KeyPressed { keyboard_key: key, modifier_keys })
    }

    /// Inserts committed text at the text tool's cursor.
    pub fn handle_text(&mut self, text: &str) -> EditState {
        self.handle_keyboard_event(PenEvent::Text { text: text.to_owned() })
    }

    fn handle_keyboard_event(&mut self, event: PenEvent) -> EditState {
        let bounds_before = self.bounds();
        // No pen mode: keys go to whatever pen is active.
        let (_propagation, flags) = self.engine.handle_pen_event(event, None, Instant::now());
        self.finish_edit(flags, bounds_before)
    }

    /// Undoes the last change, if any.
    pub fn undo(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = if self.engine.can_undo() {
            self.engine.undo(Instant::now())
        } else {
            WidgetFlags::default()
        };
        self.finish_edit(flags, bounds_before)
    }

    /// Redoes the last undone change, if any.
    pub fn redo(&mut self) -> EditState {
        let bounds_before = self.bounds();
        let flags = if self.engine.can_redo() {
            self.engine.redo(Instant::now())
        } else {
            WidgetFlags::default()
        };
        self.finish_edit(flags, bounds_before)
    }

    /// Makes `style` the tool of the pen side, if it isn't already.
    fn use_pen_style(&mut self, style: PenStyle) -> WidgetFlags {
        if self.pen_style == style {
            return WidgetFlags::default();
        }
        self.pen_style = style;
        // Reinstalls the pen and clears the selection, like the desktop app does.
        self.ensure_pen_mode() | self.engine.change_pen_style(style)
    }

    /// Switches the engine to the pen side of the stylus.
    ///
    /// Style changes apply to the current pen mode, which is the eraser after erasing with
    /// the stylus' eraser end.
    pub(super) fn ensure_pen_mode(&mut self) -> WidgetFlags {
        let (_propagation, flags) =
            self.engine.handle_pen_event(PenEvent::Cancel, Some(PenMode::Pen), Instant::now());
        flags
    }
}
