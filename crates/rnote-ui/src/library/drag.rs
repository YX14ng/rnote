//! Moving notebooks by dragging their cards onto a folder of the sidebar.

use gtk4::{gdk, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::{Notebook, Section};

use super::RnLibraryView;
use super::card::Card;

/// The folder (`""` for none) that notebooks dropped on the row of `section` go to;
/// `None` where they can't be dropped.
pub(super) fn drop_folder(section: &Section) -> Option<&str> {
    match section {
        Section::Folder(folder) => Some(folder),
        Section::Uncategorized => Some(""),
        _ => None,
    }
}

/// The folder (`""` for none) of the notebook at `path`.
fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

impl RnLibraryView {
    /// Lets `card` be dragged onto a folder, carrying the path of `notebook`.
    pub(super) fn make_draggable(&self, card: &Card, notebook: &Notebook) {
        let path = notebook.path.clone();
        let source = gtk4::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .build();
        source.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&path.to_value()))
        });
        let widget = card.widget.clone();
        source.connect_drag_begin(move |source, _| {
            let icon = gtk4::WidgetPaintable::new(Some(&widget));
            source.set_icon(Some(&icon), 0, 0);
        });
        card.widget.add_controller(source);
    }

    /// Lets notebooks be dropped on the folders of the sidebar, which moves them there.
    pub(super) fn setup_sidebar_drop(&self) {
        let list = self.imp().sidebar_list.get();
        let target = gtk4::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
        target.connect_motion(clone!(
            #[weak(rename_to = view)]
            self,
            #[weak]
            list,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, y| match view.folder_row_at(y) {
                Some((row, _)) => {
                    list.drag_highlight_row(&row);
                    gdk::DragAction::MOVE
                }
                None => {
                    list.drag_unhighlight_row();
                    gdk::DragAction::empty()
                }
            }
        ));
        target.connect_leave(clone!(
            #[weak]
            list,
            move |_| list.drag_unhighlight_row()
        ));
        target.connect_drop(clone!(
            #[weak(rename_to = view)]
            self,
            #[weak]
            list,
            #[upgrade_or]
            false,
            move |_, value, _, y| {
                list.drag_unhighlight_row();
                let (Ok(path), Some((_, folder))) = (value.get::<String>(), view.folder_row_at(y))
                else {
                    return false;
                };
                if folder_of(&path) != folder {
                    view.move_notebooks(&[path], &folder);
                }
                true
            }
        ));
        list.add_controller(target);
    }

    /// The row at `y` of the sidebar and its folder, if notebooks can be dropped on it.
    fn folder_row_at(&self, y: f64) -> Option<(gtk4::ListBoxRow, String)> {
        let imp = self.imp();
        let row = imp.sidebar_list.row_at_y(y as i32)?;
        let index = usize::try_from(row.index()).ok()?;
        let sections = imp.row_sections.borrow();
        let folder = drop_folder(sections.get(index)?.as_ref()?)?.to_owned();
        Some((row, folder))
    }
}

#[cfg(test)]
mod tests {
    use rnote_library::Section;

    use super::{drop_folder, folder_of};

    #[test]
    fn folders_and_uncategorized_take_dropped_notebooks() {
        assert_eq!(
            drop_folder(&Section::Folder("Álgebra".to_owned())),
            Some("Álgebra")
        );
        assert_eq!(drop_folder(&Section::Uncategorized), Some(""));
    }

    #[test]
    fn the_folder_of_a_notebook_is_before_its_name() {
        assert_eq!(folder_of("Álgebra/Parcial 1.rnote"), "Álgebra");
        assert_eq!(folder_of("suelto.rnote"), "");
    }

    #[test]
    fn other_sections_do_not() {
        assert_eq!(drop_folder(&Section::All), None);
        assert_eq!(drop_folder(&Section::Trash), None);
        assert_eq!(drop_folder(&Section::Tag("t".to_owned())), None);
    }
}
