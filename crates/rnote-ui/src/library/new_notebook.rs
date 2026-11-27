//! The "new notebook" dialog: title, folder, page, paper and cover, with a preview. It
//! starts from the setup last used in the folder, as on Android.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use adw::prelude::*;
use futures::channel::oneshot;
use gettextrs::gettext;
use gtk4::{glib, glib::clone};
use rnote_library::creation::{
    PAGE_SIZES, PAPERS, SPACINGS_MM, SetupChoice, SetupLayout, SetupPattern,
};
use rnote_library::{COVER_COLORS, Tag};

use super::paper_preview::PaperPreview;
use crate::RnAppWindow;

/// What the dialog chose.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NewNotebook {
    /// The title typed; empty for "Untitled".
    pub(crate) title: String,
    /// The folder, `None` for none.
    pub(crate) folder: Option<String>,
    /// The page, paper and cover.
    pub(crate) choice: SetupChoice,
    /// The ids of the tags to give it.
    pub(crate) tags: Vec<String>,
}

const LAYOUTS: [SetupLayout; 3] = [
    SetupLayout::FixedSize,
    SetupLayout::ContinuousVertical,
    SetupLayout::Infinite,
];
const PATTERNS: [SetupPattern; 4] = [
    SetupPattern::None,
    SetupPattern::Lines,
    SetupPattern::Grid,
    SetupPattern::Dots,
];

/// The rows of the dialog.
struct Rows {
    title: adw::EntryRow,
    folder: adw::ComboRow,
    size: adw::ComboRow,
    orientation: adw::ComboRow,
    layout: adw::ComboRow,
    paper: adw::ComboRow,
    pattern: adw::ComboRow,
    spacing: adw::ComboRow,
    cover: adw::ComboRow,
}

impl Rows {
    /// What the rows show now; `folders` are the folders the folder row lists after
    /// "Uncategorized".
    fn chosen(&self, folders: &[String]) -> NewNotebook {
        let index = |row: &adw::ComboRow| row.selected() as usize;
        let folder = index(&self.folder)
            .checked_sub(1)
            .and_then(|i| folders.get(i).cloned());
        NewNotebook {
            title: self.title.text().trim().to_owned(),
            folder,
            tags: Vec::new(),
            choice: SetupChoice {
                size: index(&self.size),
                landscape: index(&self.orientation) == 1,
                layout: LAYOUTS
                    .get(index(&self.layout))
                    .copied()
                    .unwrap_or(SetupLayout::FixedSize),
                paper: index(&self.paper),
                pattern: PATTERNS
                    .get(index(&self.pattern))
                    .copied()
                    .unwrap_or(SetupPattern::None),
                spacing: index(&self.spacing),
                cover: index(&self.cover)
                    .checked_sub(1)
                    .and_then(|i| COVER_COLORS.get(i).copied()),
            },
        }
    }
}

/// Asks how the new notebook should be, starting from `initial` in `folder`. `None` when
/// cancelled.
pub(crate) async fn ask_new_notebook(
    appwindow: &RnAppWindow,
    folders: &[String],
    tags: &[Tag],
    folder: Option<&str>,
    initial: SetupChoice,
) -> Option<NewNotebook> {
    let rows = Rc::new(rows(folders, folder, &initial));
    let chosen_tags: Rc<RefCell<BTreeSet<String>>> = Rc::default();
    let tags_row = tags_row(appwindow, tags, &chosen_tags);
    let preview = PaperPreview::new();
    preview.show(&initial);

    let page = adw::PreferencesPage::new();
    // A group of its own: a group shows its rows before any other widget.
    let pictured = adw::PreferencesGroup::new();
    pictured.add(preview.widget());
    page.add(&pictured);
    let top = adw::PreferencesGroup::new();
    top.add(&rows.title);
    top.add(&rows.folder);
    page.add(&top);
    page.add(&group(
        &gettext("Page"),
        &[&rows.size, &rows.orientation, &rows.layout],
    ));
    page.add(&group(
        &gettext("Paper"),
        &[&rows.paper, &rows.pattern, &rows.spacing],
    ));
    let library = group(&gettext("Library"), &[&rows.cover]);
    library.add(&tags_row);
    library.set_description(Some(&gettext(
        "What you choose stays for the next notebook in this folder.",
    )));
    page.add(&library);

    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    let create = gtk4::Button::with_label(&gettext("Create"));
    create.add_css_class("suggested-action");
    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    header.pack_start(&cancel);
    header.pack_end(&create);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title(gettext("New Notebook"))
        .content_width(440)
        .content_height(640)
        .child(&toolbar)
        .default_widget(&create)
        .focus_widget(&rows.title)
        .build();

    let folders = folders.to_vec();
    let (sender, receiver) = oneshot::channel();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let answer = clone!(
        #[strong]
        sender,
        move |chosen: Option<NewNotebook>| {
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(chosen);
            }
        }
    );
    for row in [
        &rows.size,
        &rows.orientation,
        &rows.paper,
        &rows.pattern,
        &rows.spacing,
        &rows.cover,
    ] {
        row.connect_selected_notify(clone!(
            #[strong]
            rows,
            #[strong]
            preview,
            #[strong]
            folders,
            move |_| {
                let choice = rows.chosen(&folders).choice;
                rows.spacing
                    .set_sensitive(choice.pattern != SetupPattern::None);
                preview.show(&choice);
            }
        ));
    }
    rows.spacing
        .set_sensitive(initial.pattern != SetupPattern::None);
    let accept = clone!(
        #[strong]
        rows,
        #[strong]
        answer,
        #[weak]
        dialog,
        move || {
            let mut chosen = rows.chosen(&folders);
            chosen.tags = chosen_tags.borrow().iter().cloned().collect();
            answer(Some(chosen));
            dialog.close();
        }
    );
    create.connect_clicked(clone!(
        #[strong]
        accept,
        move |_| accept()
    ));
    rows.title.connect_entry_activated(move |_| accept());
    cancel.connect_clicked(clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    dialog.connect_closed(move |_| answer(None));
    dialog.present(Some(appwindow));
    receiver.await.ok().flatten()
}

/// The row with the tags to give the new notebook; choosing them opens the tags dialog.
fn tags_row(
    appwindow: &RnAppWindow,
    tags: &[Tag],
    chosen: &Rc<RefCell<BTreeSet<String>>>,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(gettext("Tags")).build();
    if tags.is_empty() {
        row.set_subtitle(&gettext(
            "There are no tags yet: create them from the library's sidebar.",
        ));
        return row;
    }
    row.set_subtitle(&gettext("No tags"));
    row.set_activatable(true);
    row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
    let tags = tags.to_vec();
    row.connect_activated(clone!(
        #[weak]
        appwindow,
        #[strong]
        chosen,
        move |row| {
            let tags = tags.clone();
            let chosen = chosen.clone();
            glib::spawn_future_local(clone!(
                #[weak]
                appwindow,
                #[weak]
                row,
                async move {
                    let current = chosen.borrow().clone();
                    let Some(checked) =
                        super::dialogs::choose_tags(&appwindow, &tags, &current).await
                    else {
                        return;
                    };
                    let names: Vec<&str> = tags
                        .iter()
                        .filter(|tag| checked.contains(&tag.id))
                        .map(|tag| tag.name.as_str())
                        .collect();
                    row.set_subtitle(&if names.is_empty() {
                        gettext("No tags")
                    } else {
                        names.join(", ")
                    });
                    *chosen.borrow_mut() = checked;
                }
            ));
        }
    ));
    row
}

fn rows(folders: &[String], folder: Option<&str>, initial: &SetupChoice) -> Rows {
    let title = adw::EntryRow::builder()
        .title(gettext("Title"))
        .activates_default(true)
        .show_apply_button(false)
        .build();
    let mut folder_names = vec![gettext("Uncategorized")];
    folder_names.extend(folders.iter().cloned());
    let folder_index = folder
        .and_then(|folder| folders.iter().position(|f| f == folder))
        .map_or(0, |i| i + 1);
    let sizes = size_labels();
    let papers = [
        gettext("White"),
        gettext("Cream"),
        gettext("Light Gray"),
        gettext("Dark"),
    ];
    debug_assert_eq!(papers.len(), PAPERS.len());
    let spacings: Vec<String> = SPACINGS_MM
        .iter()
        .map(|mm| gettext("{n} mm").replace("{n}", &mm.to_string()))
        .collect();
    let mut covers = vec![gettext("No Cover")];
    covers.extend(
        COVER_COLORS
            .iter()
            .map(|argb| super::covers::cover_name(*argb)),
    );
    let layout_index = LAYOUTS
        .iter()
        .position(|l| *l == initial.layout)
        .unwrap_or(0);
    let pattern_index = PATTERNS
        .iter()
        .position(|p| *p == initial.pattern)
        .unwrap_or(0);
    let cover_index = initial
        .cover
        .and_then(|cover| COVER_COLORS.iter().position(|c| *c == cover))
        .map_or(0, |i| i + 1);
    Rows {
        title,
        folder: combo(&gettext("Folder"), &folder_names, folder_index),
        size: combo(&gettext("Format"), &sizes, initial.size),
        orientation: combo(
            &gettext("Orientation"),
            &[gettext("Portrait"), gettext("Landscape")],
            usize::from(initial.landscape),
        ),
        layout: combo(
            &gettext("Pages"),
            &[
                gettext("Sheets"),
                gettext("Continuous"),
                gettext("Infinite"),
            ],
            layout_index,
        ),
        paper: combo(&gettext("Paper Color"), &papers, initial.paper),
        pattern: combo(
            &gettext("Template"),
            &[
                gettext("Blank"),
                gettext("Ruled"),
                gettext("Grid"),
                gettext("Dots"),
            ],
            pattern_index,
        ),
        spacing: combo(&gettext("Line Spacing"), &spacings, initial.spacing),
        cover: combo(&gettext("Cover"), &covers, cover_index),
    }
}

/// The names of the page sizes: A3, A4, A5, Letter and Legal.
pub(super) fn size_names() -> [String; 5] {
    [
        "A3".to_owned(),
        "A4".to_owned(),
        "A5".to_owned(),
        gettext("Letter"),
        gettext("Legal"),
    ]
}

/// The names of the page sizes, with their measures, e.g. "A4 · 210 × 297 mm".
pub(super) fn size_labels() -> Vec<String> {
    size_names()
        .iter()
        .zip(PAGE_SIZES)
        .map(|(name, size)| format!("{name} · {} × {} mm", size.width_mm, size.height_mm))
        .collect()
}

/// A row to choose one of `labels`, with `selected` chosen.
pub(crate) fn combo(title: &str, labels: &[String], selected: usize) -> adw::ComboRow {
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&gtk4::StringList::new(&labels))
        .build();
    row.set_selected(u32::try_from(selected).unwrap_or(0));
    row
}

fn group(title: &str, rows: &[&adw::ComboRow]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    for row in rows {
        group.add(*row);
    }
    group
}
