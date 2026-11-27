//! What the page menus do: insert, duplicate, move and remove pages. Each is one step to
//! undo.

use gettextrs::gettext;
use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
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
