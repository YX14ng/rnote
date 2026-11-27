//! The library's keyboard shortcuts: search, rename, trash, select all and refresh.

use std::collections::BTreeSet;

use gtk4::{glib, prelude::*, subclass::prelude::*};

use super::{RnLibraryView, SELECTION_MODE};

/// The notebooks a shortcut acts on: the selected ones in selection mode, otherwise the
/// one whose card has the keyboard focus.
fn targets(selecting: bool, selected: &BTreeSet<String>, focused: Option<String>) -> Vec<String> {
    if selecting {
        selected.iter().cloned().collect()
    } else {
        focused.into_iter().collect()
    }
}

impl RnLibraryView {
    /// Adds the shortcuts, which work while the keyboard focus is in the library.
    pub(super) fn setup_shortcuts(&self) {
        let controller = gtk4::ShortcutController::new();
        // Before the grid, whose own Ctrl+A would take the key.
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let add = |triggers: &str, run: fn(&RnLibraryView) -> bool| {
            let action = gtk4::CallbackAction::new(move |widget, _| {
                match widget.downcast_ref::<RnLibraryView>() {
                    // Delete, Ctrl+A and Escape belong to the text while typing a search.
                    Some(view) if !view.typing() && run(view) => glib::Propagation::Stop,
                    _ => glib::Propagation::Proceed,
                }
            });
            for trigger in triggers.split('|') {
                let trigger = gtk4::ShortcutTrigger::parse_string(trigger);
                controller.add_shortcut(gtk4::Shortcut::new(trigger, Some(action.clone())));
            }
        };
        add("<Control>f", Self::start_search);
        add("F2", Self::rename_target);
        add("Delete|KP_Delete", Self::trash_targets);
        add("<Control>a", Self::select_all);
        add("Escape", Self::leave_selection_mode);
        add("F5|<Control>r", |view| {
            view.refresh();
            true
        });
        self.add_controller(controller);
    }

    fn start_search(&self) -> bool {
        let imp = self.imp();
        imp.search_bar.set_search_mode(true);
        imp.search_entry.grab_focus();
        true
    }

    fn rename_target(&self) -> bool {
        match self.shortcut_targets().as_slice() {
            [path] => {
                self.imp()
                    .actions
                    .activate_action("rename", Some(&path.to_variant()));
                true
            }
            _ => false,
        }
    }

    fn trash_targets(&self) -> bool {
        let paths = self.shortcut_targets();
        let actions = &self.imp().actions;
        match paths.as_slice() {
            [] => return false,
            _ if self.flag(SELECTION_MODE) => actions.activate_action("selection-trash", None),
            [path] => actions.activate_action("trash", Some(&path.to_variant())),
            _ => return false,
        }
        true
    }

    fn select_all(&self) -> bool {
        if !self.flag(SELECTION_MODE) {
            return false;
        }
        let imp = self.imp();
        let all = imp
            .shown
            .borrow()
            .iter()
            .map(|notebook| notebook.path.clone())
            .collect();
        *imp.selected.borrow_mut() = all;
        self.show_notebooks();
        true
    }

    fn leave_selection_mode(&self) -> bool {
        if !self.flag(SELECTION_MODE) {
            return false;
        }
        self.set_selection_mode(false);
        true
    }

    /// Whether the keyboard focus is in a text field.
    fn typing(&self) -> bool {
        WidgetExt::root(self)
            .and_then(|root| root.focus())
            .is_some_and(|focus| focus.is::<gtk4::Text>() || focus.is::<gtk4::TextView>())
    }

    fn shortcut_targets(&self) -> Vec<String> {
        let imp = self.imp();
        targets(
            self.flag(SELECTION_MODE),
            &imp.selected.borrow(),
            self.focused_notebook(),
        )
    }

    /// The path of the notebook whose card has the keyboard focus.
    fn focused_notebook(&self) -> Option<String> {
        let imp = self.imp();
        // `Self::root` is the library's folder; this is the window.
        let focus = WidgetExt::root(self)?.focus()?;
        let child = focus
            .ancestor(gtk4::FlowBoxChild::static_type())?
            .downcast::<gtk4::FlowBoxChild>()
            .ok()?;
        if !child.is_ancestor(&*imp.flowbox) {
            return None;
        }
        let index = usize::try_from(child.index()).ok()?;
        let index = index.checked_sub(usize::from(imp.new_card_shown.get()))?;
        imp.shown
            .borrow()
            .get(index)
            .map(|notebook| notebook.path.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::targets;

    fn set(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn outside_selection_mode_the_focused_card_is_the_target() {
        assert_eq!(targets(false, &set(&["a"]), Some("b".to_owned())), ["b"]);
        assert!(targets(false, &set(&["a"]), None).is_empty());
    }

    #[test]
    fn in_selection_mode_the_selected_notebooks_are_the_targets() {
        assert_eq!(
            targets(true, &set(&["a", "c"]), Some("b".to_owned())),
            ["a", "c"]
        );
    }
}
