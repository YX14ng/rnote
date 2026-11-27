//! The notice after sending notebooks to the trash, with a button that brings them back.

use std::time::Duration;

use gettextrs::{gettext, ngettext};
use gtk4::{glib, glib::clone, subclass::prelude::*};

use super::RnLibraryView;

/// How long the notice stays, so there is time to change one's mind.
const UNDO_TIMEOUT: Duration = Duration::from_secs(8);

/// What the notice says after the notebooks named `names` went to the trash.
pub(super) fn trashed_message(names: &[String]) -> String {
    match names {
        [name] => gettext("“{name}” was moved to the trash").replace("{name}", name),
        _ => {
            let count = u32::try_from(names.len()).unwrap_or(u32::MAX);
            ngettext(
                "{n} notebook was moved to the trash",
                "{n} notebooks were moved to the trash",
                count,
            )
            .replace("{n}", &names.len().to_string())
        }
    }
}

impl RnLibraryView {
    /// Tells that the notebooks in `trashed` (name and trash id) went to the trash, with a
    /// button that restores them. A newer notice replaces the previous one.
    pub(super) fn offer_undo_trash(&self, trashed: Vec<(String, String)>) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        if trashed.is_empty() {
            return;
        }
        let names: Vec<String> = trashed.iter().map(|(name, _)| name.clone()).collect();
        let ids: Vec<String> = trashed.into_iter().map(|(_, id)| id).collect();
        appwindow.overlays().dispatch_toast_w_button_singleton(
            &trashed_message(&names),
            &gettext("Undo"),
            clone!(
                #[weak(rename_to = view)]
                self,
                move |toast| {
                    toast.dismiss();
                    view.modify_each(ids.clone(), |library, id| library.restore(&id));
                }
            ),
            Some(UNDO_TIMEOUT),
            &self.imp().trash_toast,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::trashed_message;

    #[test]
    fn one_notebook_is_named() {
        assert_eq!(
            trashed_message(&["Álgebra".to_owned()]),
            "“Álgebra” was moved to the trash"
        );
    }

    #[test]
    fn several_notebooks_are_counted() {
        let names = ["a".to_owned(), "b".to_owned(), "c".to_owned()];

        assert_eq!(
            trashed_message(&names),
            "3 notebooks were moved to the trash"
        );
    }
}
