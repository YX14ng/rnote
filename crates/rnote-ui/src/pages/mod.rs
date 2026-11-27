//! The column of pages beside the editor: a thumbnail of each sheet of the notebook shown,
//! to go to a page and to insert, duplicate, move or remove pages, as on Android.

mod actions;
mod insert_pdf;

use std::cell::Cell;
use std::time::Duration;

use gettextrs::{gettext, ngettext};
use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use p2d::math::Vector2;
use rnote_compose::SplitOrder;
use rnote_engine::engine::StrokeContent;

use crate::{RnAppWindow, RnCanvas, StrokeContentPaintable};

/// The largest size of a thumbnail, in pixels.
const THUMBNAIL_WIDTH: f64 = 160.0;
const THUMBNAIL_HEIGHT: f64 = 230.0;
/// How long to wait for more changes before drawing the thumbnails again.
const SETTLE: Duration = Duration::from_millis(400);

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct RnPagesPanel {
        pub(crate) appwindow: glib::WeakRef<RnAppWindow>,
        pub(crate) stack: gtk4::Stack,
        pub(crate) list: gtk4::ListBox,
        pub(crate) count_label: gtk4::Label,
        pub(crate) actions: gio::SimpleActionGroup,
        pub(crate) refresh_pending: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RnPagesPanel {
        const NAME: &'static str = "RnPagesPanel";
        type Type = super::RnPagesPanel;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk4::BinLayout>();
        }
    }

    impl ObjectImpl for RnPagesPanel {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.build();
            obj.add_actions();
        }

        fn dispose(&self) {
            self.stack.unparent();
        }
    }

    impl WidgetImpl for RnPagesPanel {}
}

glib::wrapper! {
    pub(crate) struct RnPagesPanel(ObjectSubclass<imp::RnPagesPanel>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for RnPagesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl RnPagesPanel {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    pub(crate) fn init(&self, appwindow: &RnAppWindow) {
        self.imp().appwindow.set(Some(appwindow));
    }

    fn appwindow(&self) -> Option<RnAppWindow> {
        self.imp().appwindow.upgrade()
    }

    fn canvas(&self) -> Option<RnCanvas> {
        self.appwindow()?.active_tab_canvas()
    }

    fn build(&self) {
        let imp = self.imp();
        imp.count_label.set_xalign(0.0);
        imp.count_label.set_hexpand(true);
        imp.count_label.add_css_class("dim-label");
        let add = gtk4::Button::builder()
            .icon_name("add-page-symbolic")
            .tooltip_text(gettext("Add a Page at the End"))
            .action_name("pages.add-at-end")
            .css_classes(["flat"])
            .build();
        let header = gtk4::Box::builder()
            .spacing(6)
            .margin_start(12)
            .margin_end(6)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        header.append(&imp.count_label);
        header.append(&add);
        imp.list.add_css_class("navigation-sidebar");
        imp.list.connect_row_activated(clone!(
            #[weak(rename_to = panel)]
            self,
            move |_, row| {
                if let Ok(page) = u32::try_from(row.index()) {
                    panel.go_to(page);
                }
            }
        ));
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&imp.list)
            .build();
        let pages = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        pages.append(&header);
        pages.append(&scrolled);
        let none = adw::StatusPage::builder()
            .icon_name("document-page-setup-symbolic")
            .title(gettext("No Pages"))
            .description(gettext(
                "Only notebooks of sheets have pages. Choose “Fixed Size” as the layout in the document settings.",
            ))
            .build();
        imp.stack.add_named(&pages, Some("pages"));
        imp.stack.add_named(&none, Some("none"));
        imp.stack.set_parent(self);
        self.insert_action_group("pages", Some(&imp.actions));
        self.connect_map(|panel| panel.refresh());
    }

    /// The notebook shown changed: draws the thumbnails again once the changes settle.
    pub(crate) fn changed(&self) {
        if !self.is_mapped() || self.imp().refresh_pending.replace(true) {
            return;
        }
        glib::timeout_add_local_once(
            SETTLE,
            clone!(
                #[weak(rename_to = panel)]
                self,
                move || {
                    panel.imp().refresh_pending.set(false);
                    panel.refresh();
                }
            ),
        );
    }

    /// Shows the pages of the notebook shown.
    pub(crate) fn refresh(&self) {
        let imp = self.imp();
        let pages = self.canvas().and_then(|canvas| {
            let engine = canvas.engine_ref();
            let count = engine.doc_page_count_fixed_size()?;
            let mut pages = engine.extract_all_pages_content(SplitOrder::default());
            pages.truncate(count as usize);
            Some((count, pages))
        });
        let Some((count, pages)) = pages else {
            imp.stack.set_visible_child_name("none");
            return;
        };
        imp.stack.set_visible_child_name("pages");
        imp.count_label.set_label(
            &ngettext("{n} page", "{n} pages", count).replace("{n}", &count.to_string()),
        );
        imp.list.remove_all();
        for (page, content) in (0..).zip(pages) {
            imp.list.append(&self.row(page, count, content));
        }
        self.follow();
    }

    fn row(&self, page: u32, count: u32, content: StrokeContent) -> gtk4::ListBoxRow {
        let paintable = StrokeContentPaintable::new();
        paintable.set_paint_max_width(THUMBNAIL_WIDTH);
        paintable.set_paint_max_height(THUMBNAIL_HEIGHT);
        paintable.set_stroke_content(content);
        let picture = gtk4::Picture::builder()
            .paintable(&paintable)
            .content_fit(gtk4::ContentFit::Contain)
            .can_shrink(true)
            .height_request(THUMBNAIL_HEIGHT as i32)
            .build();
        let number = gtk4::Label::builder()
            .label((page + 1).to_string())
            .xalign(0.0)
            .hexpand(true)
            .build();
        let menu = gtk4::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .menu_model(&actions::page_menu(page, count))
            .tooltip_text(gettext("Page Menu"))
            .css_classes(["flat"])
            .build();
        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        footer.append(&number);
        footer.append(&menu);
        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        content.append(&picture);
        content.append(&footer);
        gtk4::ListBoxRow::builder().child(&content).build()
    }

    /// Marks the page in the middle of the view.
    pub(crate) fn follow(&self) {
        let Some(canvas) = self.canvas() else {
            return;
        };
        let page = {
            let engine = canvas.engine_ref();
            engine.doc_page_at_fixed_size(engine.camera.viewport_center()[1])
        };
        let list = &self.imp().list;
        let row = page.and_then(|page| list.row_at_index(i32::try_from(page).ok()?));
        if row.is_some() && list.selected_row() != row {
            list.select_row(row.as_ref());
        }
    }

    /// Shows `page` from its top.
    fn go_to(&self, page: u32) {
        let (Some(appwindow), Some(canvas)) = (self.appwindow(), self.canvas()) else {
            return;
        };
        let widget_flags = {
            let mut engine = canvas.engine_mut();
            let Some(top) = engine.doc_page_top_fixed_size(page) else {
                return;
            };
            let offset = engine.camera.offset();
            let zoom = engine.camera.total_zoom();
            engine.camera_set_offset_expand(Vector2::new(offset[0], top * zoom))
        };
        appwindow.handle_widget_flags(widget_flags, &canvas);
    }
}
