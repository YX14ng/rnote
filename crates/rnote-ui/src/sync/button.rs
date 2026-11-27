//! The Google Drive button of the library's header bar, with its menu and toasts.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

use gettextrs::{gettext, ngettext};
use gtk4::{Align, Button, Label, MenuButton, Orientation, Popover, glib, glib::clone, prelude::*};
use rnote_sync::SyncReport;

use super::SyncStatus;
use super::status::status_text;
use crate::RnAppWindow;
use crate::library::dialogs;

/// The icon of `button` for `status`.
pub(crate) fn icon_name(status: &SyncStatus) -> &'static str {
    match status {
        SyncStatus::Unavailable | SyncStatus::SignedOut | SyncStatus::SignInNeeded => {
            "sync-cloud-off-symbolic"
        }
        SyncStatus::Running => "sync-cloud-sync-symbolic",
        SyncStatus::Checking | SyncStatus::Waiting | SyncStatus::Done { .. } => {
            "sync-cloud-symbolic"
        }
        SyncStatus::Pending | SyncStatus::Failed(_) | SyncStatus::KeyringLocked => {
            "sync-cloud-alert-symbolic"
        }
    }
}

/// Whether a sync that did `report` may have changed the notebooks shown.
pub(crate) fn changed_library(report: &SyncReport) -> bool {
    report.uploaded + report.downloaded + report.moved + report.deleted > 0
        || !report.conflicts.is_empty()
        || !report.changed_here.is_empty()
}

/// Makes `button` the Google Drive button of `appwindow`'s library.
pub(crate) fn setup(button: &MenuButton, appwindow: &RnAppWindow) {
    let sync = appwindow.sync();
    let popover = Popover::new();
    button.set_popover(Some(&popover));
    popover.connect_show(clone!(
        #[weak]
        appwindow,
        #[weak]
        popover,
        move |_| fill_popover(&popover, &appwindow)
    ));
    // The failure last told in a toast, until a sync works again: once per failure, not
    // once per attempt.
    let toasted: Rc<RefCell<Option<String>>> = Rc::default();
    let button = button.downgrade();
    let window = appwindow.downgrade();
    let menu = popover.downgrade();
    let show = move |status: &SyncStatus| {
        let (Some(button), Some(appwindow)) = (button.upgrade(), window.upgrade()) else {
            return;
        };
        show_status(&button, status);
        // A menu open while the sync ends shows how it went, not how it was going.
        if let Some(menu) = menu.upgrade().filter(|menu| menu.is_visible()) {
            fill_popover(&menu, &appwindow);
        }
        match status {
            SyncStatus::Done { report, .. } => {
                toasted.borrow_mut().take();
                after_sync(&appwindow, report);
            }
            SyncStatus::Failed(message) if toasted.borrow().as_ref() != Some(message) => {
                *toasted.borrow_mut() = Some(message.clone());
                appwindow
                    .overlays()
                    .dispatch_toast_error(&status_text(status, SystemTime::now()));
            }
            _ => {}
        }
    };
    show(&sync.status());
    sync.connect_status(show);
}

fn show_status(button: &MenuButton, status: &SyncStatus) {
    button.set_icon_name(icon_name(status));
    let text = status_text(status, SystemTime::now());
    button.set_tooltip_text(Some(&text));
    // Without credentials there's nothing to do, only the tooltip that says why.
    button.set_sensitive(*status != SyncStatus::Unavailable);
}

/// Reloads the library after a sync that changed it, and tells about conflict copies.
fn after_sync(appwindow: &RnAppWindow, report: &SyncReport) {
    if changed_library(report) && appwindow.library_shown() {
        appwindow.library_view().refresh();
    }
    let text = conflicts_text(&report.conflicts);
    match report.conflicts.as_slice() {
        [] => {}
        // One copy: open it right from the toast, to compare it with the other version.
        [copy] => {
            let window = appwindow.downgrade();
            let copy = copy.clone();
            appwindow.overlays().dispatch_toast_w_button(
                &text,
                &gettext("Open"),
                move |_| {
                    if let Some(appwindow) = window.upgrade() {
                        open_copy(&appwindow, &copy);
                    }
                },
                None,
            );
        }
        _ => {
            let window = appwindow.downgrade();
            appwindow.overlays().dispatch_toast_w_button(
                &text,
                &gettext("Show Library"),
                move |_| {
                    if let Some(appwindow) = window.upgrade() {
                        appwindow.show_library();
                    }
                },
                None,
            );
        }
    }
}

/// What the toast says about the conflict copies at `paths`: the copy's name when there is
/// one, how many when there are more.
fn conflicts_text(paths: &[String]) -> String {
    if let [path] = paths {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        let name = file_name
            .rsplit_once('.')
            .map_or(file_name, |(stem, _)| stem);
        return gettext("“{name}” was changed on two devices: both versions were kept")
            .replace("{name}", name);
    }
    let count = u32::try_from(paths.len()).unwrap_or(u32::MAX);
    ngettext(
        "Changed on two devices: kept both, one as a conflict copy",
        "Changed on two devices: kept both versions of {n} notebooks",
        count,
    )
    .replace("{n}", &count.to_string())
}

/// Opens the conflict copy at `path` of the library in a tab.
fn open_copy(appwindow: &RnAppWindow, path: &str) {
    let root = appwindow.library_view().root();
    let notebook = crate::library::open::open_existing_library(&root)
        .and_then(|library| library.notebooks())
        .ok()
        .and_then(|notebooks| notebooks.into_iter().find(|n| n.path == path));
    let Some(notebook) = notebook else {
        appwindow
            .overlays()
            .dispatch_toast_error(&gettext("Opening file failed"));
        return;
    };
    let appwindow = appwindow.clone();
    glib::spawn_future_local(async move {
        crate::library::open::open_notebook(&appwindow, &root, &notebook).await;
    });
}

/// The menu's status line and the actions that fit it.
fn fill_popover(popover: &Popover, appwindow: &RnAppWindow) {
    let sync = appwindow.sync();
    let status = sync.status();
    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    let label = Label::builder()
        .label(status_text(&status, SystemTime::now()))
        .wrap(true)
        .max_width_chars(32)
        .halign(Align::Start)
        .margin_bottom(6)
        .build();
    label.add_css_class("dim-label");
    content.append(&label);
    if status.signed_in() {
        content.append(&action(popover, &gettext("Sync Now"), false, {
            let sync = sync.clone();
            move || sync.sync_now()
        }));
        content.append(&action(
            popover,
            &gettext("Sign Out of Google Drive"),
            false,
            {
                let window = appwindow.downgrade();
                move || {
                    if let Some(appwindow) = window.upgrade() {
                        glib::spawn_future_local(sign_out(appwindow));
                    }
                }
            },
        ));
    } else if status == SyncStatus::KeyringLocked {
        // Signing in again would fail to keep the sign-in: unlocking is what's missing.
        content.append(&action(popover, &gettext("Retry"), true, {
            let sync = sync.clone();
            move || sync.retry_keyring()
        }));
    } else {
        content.append(&action(popover, &gettext("Sign In with Google"), true, {
            let window = appwindow.downgrade();
            move || {
                if let Some(appwindow) = window.upgrade() {
                    sign_in(&appwindow);
                }
            }
        }));
    }
    popover.set_child(Some(&content));
}

fn action(popover: &Popover, label: &str, suggested: bool, run: impl Fn() + 'static) -> Button {
    let button = Button::with_label(label);
    button.add_css_class(if suggested {
        "suggested-action"
    } else {
        "flat"
    });
    let popover = popover.downgrade();
    button.connect_clicked(move |_| {
        if let Some(popover) = popover.upgrade() {
            popover.popdown();
        }
        run();
    });
    button
}

fn sign_in(appwindow: &RnAppWindow) {
    let window = appwindow.downgrade();
    appwindow
        .sync()
        .sign_in(appwindow.upcast_ref(), move |result| {
            if let (Err(error), Some(appwindow)) = (result, window.upgrade()) {
                appwindow.overlays().dispatch_toast_error(
                    &gettext("Signing in to Google Drive failed: {error}")
                        .replace("{error}", &error),
                );
            }
        });
}

async fn sign_out(appwindow: RnAppWindow) {
    let confirmed = dialogs::confirm(
        &appwindow,
        &gettext("Sign Out of Google Drive?"),
        &gettext(
            "The notebooks stay on this computer and in your Drive. Syncing stops until you sign in again.",
        ),
        &gettext("Sign Out"),
    )
    .await;
    if !confirmed {
        return;
    }
    let window = appwindow.downgrade();
    appwindow.sync().sign_out(move |result| {
        if let (Err(error), Some(appwindow)) = (result, window.upgrade()) {
            appwindow.overlays().dispatch_toast_error(
                &gettext("Signed out here, but Google couldn't be told: {error}")
                    .replace("{error}", &error),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use rnote_sync::SyncReport;

    use super::{changed_library, conflicts_text, icon_name};
    use crate::sync::SyncStatus;

    #[test]
    fn one_conflict_is_named() {
        let text = conflicts_text(&[
            "Orga/Parcial (conflicto, SM-X920, 2026-11-12 17.20).rnote".to_owned()
        ]);

        assert_eq!(
            text,
            "“Parcial (conflicto, SM-X920, 2026-11-12 17.20)” was changed on two devices: both versions were kept"
        );
    }

    #[test]
    fn many_conflicts_are_counted() {
        let text = conflicts_text(&["a.rnote".to_owned(), "b.rnote".to_owned()]);

        assert_eq!(
            text,
            "Changed on two devices: kept both versions of 2 notebooks"
        );
    }

    #[test]
    fn sync_button_icon_per_status() {
        let done = SyncStatus::Done {
            at: SystemTime::now(),
            report: SyncReport::default(),
        };

        assert_eq!(
            icon_name(&SyncStatus::Unavailable),
            "sync-cloud-off-symbolic"
        );
        assert_eq!(icon_name(&SyncStatus::SignedOut), "sync-cloud-off-symbolic");
        assert_eq!(
            icon_name(&SyncStatus::SignInNeeded),
            "sync-cloud-off-symbolic"
        );
        assert_eq!(icon_name(&SyncStatus::Waiting), "sync-cloud-symbolic");
        assert_eq!(icon_name(&SyncStatus::Checking), "sync-cloud-symbolic");
        assert_eq!(icon_name(&SyncStatus::Running), "sync-cloud-sync-symbolic");
        assert_eq!(icon_name(&done), "sync-cloud-symbolic");
        assert_eq!(icon_name(&SyncStatus::Pending), "sync-cloud-alert-symbolic");
        assert_eq!(
            icon_name(&SyncStatus::Failed("x".into())),
            "sync-cloud-alert-symbolic"
        );
    }

    #[test]
    fn any_work_changes_the_library_shown() {
        let report = SyncReport::default;

        assert!(!changed_library(&report()));
        assert!(changed_library(&SyncReport {
            uploaded: 1,
            ..report()
        }));
        assert!(changed_library(&SyncReport {
            downloaded: 1,
            ..report()
        }));
        assert!(changed_library(&SyncReport {
            moved: 1,
            ..report()
        }));
        assert!(changed_library(&SyncReport {
            deleted: 1,
            ..report()
        }));
        assert!(changed_library(&SyncReport {
            conflicts: vec!["a.rnote".into()],
            ..report()
        }));
        assert!(changed_library(&SyncReport {
            changed_here: vec!["a.rnote".into()],
            ..report()
        }));
    }
}
