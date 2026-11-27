//! What the page menus do: insert, duplicate, move and remove pages. Each is one step to
//! undo.

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk4::{gio, glib, glib::clone, subclass::prelude::*};
use rnote_engine::{Engine, WidgetFlags};

use super::RnPagesPanel;

/// The menu of `page` (0-based) of a notebook of `count` pages.
pub(super) fn page_menu(page: u32, count: u32) -> gio::Menu {
    let item = |label: &str, action: &str| {
        let item = gio::MenuItem::new(Some(label), None);
        item.set_action_and_target_value(Some(action), Some(&page.to_variant()));
        item
    };
    let insert = gio::Menu::new();
    insert.append_item(&item(
        &gettext("Insert Blank Page After"),
        "pages.insert-blank",
    ));
    insert.append_item(&item(&gettext("Insert PDF After…"), "pages.insert-pdf"));
    let arrange = gio::Menu::new();
    arrange.append_item(&item(&gettext("Duplicate"), "pages.duplicate"));
    if page > 0 {
        arrange.append_item(&item(&gettext("Move Up"), "pages.move-up"));
    }
    if page + 1 < count {
        arrange.append_item(&item(&gettext("Move Down"), "pages.move-down"));
    }
    let remove = gio::Menu::new();
    remove.append_item(&item(&gettext("Remove Page"), "pages.remove"));
    if count > 1 {
        remove.append_item(&item(&gettext("Remove Pages…"), "pages.remove-several"));
    }
    let menu = gio::Menu::new();
    menu.append_section(None, &insert);
    menu.append_section(None, &arrange);
    menu.append_section(None, &remove);
    menu
}

impl RnPagesPanel {
    pub(super) fn add_actions(&self) {
        let actions = &self.imp().actions;
        let page = Some(glib::VariantTy::UINT32);
        let add = |name: &str, run: fn(&RnPagesPanel, u32)| {
            let action = gio::SimpleAction::new(name, page);
            action.connect_activate(clone!(
                #[weak(rename_to = panel)]
                self,
                move |_, arg| {
                    if let Some(page) = arg.and_then(|arg| arg.get::<u32>()) {
                        run(&panel, page);
                    }
                }
            ));
            actions.add_action(&action);
        };
        add("insert-blank", |panel, page| {
            panel.edit(|engine| engine.doc_insert_pages_fixed_size(page + 1, 1));
        });
        add("duplicate", |panel, page| {
            panel.edit(|engine| engine.doc_duplicate_page_fixed_size(page));
        });
        add("move-up", |panel, page| {
            panel.edit(|engine| engine.doc_move_page_fixed_size(page, page.saturating_sub(1)));
        });
        add("move-down", |panel, page| {
            panel.edit(|engine| engine.doc_move_page_fixed_size(page, page + 1));
        });
        add("remove", |panel, page| {
            glib::spawn_future_local(clone!(
                #[weak]
                panel,
                async move { panel.remove(page).await }
            ));
        });
        add("remove-several", |panel, page| {
            glib::spawn_future_local(clone!(
                #[weak]
                panel,
                async move { panel.remove_several(page).await }
            ));
        });
        add("insert-pdf", |panel, page| {
            glib::spawn_future_local(clone!(
                #[weak]
                panel,
                async move { panel.insert_pdf(page + 1).await }
            ));
        });
        let at_end = gio::SimpleAction::new("add-at-end", None);
        at_end.connect_activate(clone!(
            #[weak(rename_to = panel)]
            self,
            move |_, _| {
                panel.edit(|engine| {
                    let count = engine.doc_page_count_fixed_size().unwrap_or(0);
                    engine.doc_insert_pages_fixed_size(count, 1)
                });
            }
        ));
        actions.add_action(&at_end);
    }

    /// Runs `op` on the notebook shown and shows the result.
    pub(super) fn edit(&self, op: impl FnOnce(&mut Engine) -> WidgetFlags) {
        let (Some(appwindow), Some(canvas)) = (self.appwindow(), self.canvas()) else {
            return;
        };
        let widget_flags = op(&mut canvas.engine_mut());
        appwindow.handle_widget_flags(widget_flags, &canvas);
        self.refresh();
    }

    /// Asks which pages to remove, starting from `page`, then removes them, asking first if
    /// something is written on them; one step to undo, as on Android.
    async fn remove_several(&self, page: u32) {
        let (Some(appwindow), Some(canvas)) = (self.appwindow(), self.canvas()) else {
            return;
        };
        let count = canvas.engine_ref().doc_page_count_fixed_size().unwrap_or(0);
        let Some((from, to)) = ask_pages_to_remove(&appwindow, page + 1, count).await else {
            return;
        };
        let pages = pages_between(from, to, count);
        let with_content = canvas.engine_ref().doc_pages_with_content_fixed_size();
        let written = pages
            .iter()
            .any(|&page| with_content.get(page as usize).copied().unwrap_or(false));
        let n = pages.len() as u32;
        if written
            && !crate::library::dialogs::confirm(
                &appwindow,
                &ngettext("Remove {n} Page?", "Remove {n} Pages?", n)
                    .replace("{n}", &n.to_string()),
                &gettext("What is written on them is removed too. You can undo it."),
                &gettext("Remove"),
            )
            .await
        {
            return;
        }
        self.edit(|engine| engine.doc_remove_pages_fixed_size(&pages));
    }

    /// Removes `page`, asking first if something is written on it.
    async fn remove(&self, page: u32) {
        let (Some(appwindow), Some(canvas)) = (self.appwindow(), self.canvas()) else {
            return;
        };
        let with_content = canvas
            .engine_ref()
            .doc_pages_with_content_fixed_size()
            .get(page as usize)
            .copied()
            .unwrap_or(false);
        if with_content
            && !crate::library::dialogs::confirm(
                &appwindow,
                &gettext("Remove the Page?"),
                &gettext("What is written on it is removed too. You can undo it."),
                &gettext("Remove"),
            )
            .await
        {
            return;
        }
        self.edit(|engine| engine.doc_remove_pages_fixed_size(&[page]));
    }
}

/// The pages (from 0) of a notebook of `count` pages between the numbers `from` and `to`
/// (as shown, from 1), in either order.
fn pages_between(from: u32, to: u32, count: u32) -> Vec<u32> {
    let last = count.max(1);
    let (first, end) = (from.min(to).clamp(1, last), from.max(to).clamp(1, last));
    (first - 1..end).collect()
}

/// Asks from which page to which (numbers as shown) to remove, in a notebook of `count`
/// pages, starting at `page`. `None` when cancelled.
async fn ask_pages_to_remove(
    appwindow: &crate::RnAppWindow,
    page: u32,
    count: u32,
) -> Option<(u32, u32)> {
    const REMOVE: &str = "remove";
    const CANCEL: &str = "cancel";
    let count_f = f64::from(count);
    let first_row = adw::SpinRow::with_range(1.0, count_f, 1.0);
    first_row.set_title(&gettext("From Page"));
    first_row.set_value(f64::from(page));
    let last_row = adw::SpinRow::with_range(1.0, count_f, 1.0);
    last_row.set_title(&gettext("To Page"));
    last_row.set_value(f64::from(page));
    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&first_row);
    list.append(&last_row);
    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Remove Pages"))
        .extra_child(&list)
        .default_response(REMOVE)
        .close_response(CANCEL)
        .build();
    dialog.add_responses(&[(CANCEL, &gettext("Cancel")), (REMOVE, &gettext("Remove"))]);
    dialog.set_response_appearance(REMOVE, adw::ResponseAppearance::Destructive);
    if dialog.choose_future(Some(appwindow)).await != REMOVE {
        return None;
    }
    Some((first_row.value() as u32, last_row.value() as u32))
}

#[cfg(test)]
mod tests {
    use super::pages_between;

    #[test]
    fn the_pages_between_two_numbers_in_either_order_inside_the_notebook() {
        // Numbers as shown (from 1), pages from 0.
        assert_eq!(pages_between(2, 4, 5), vec![1, 2, 3]);
        assert_eq!(pages_between(4, 2, 5), vec![1, 2, 3]);
        assert_eq!(pages_between(3, 9, 5), vec![2, 3, 4]);
        assert_eq!(pages_between(0, 1, 5), vec![0]);
    }
}
