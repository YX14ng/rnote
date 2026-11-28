# Rnote (YX14ng) on the desktop

The fork's desktop build adds a **library** home screen to Rnote: the notebooks of one
folder, with folders, tags, a trash, thumbnails and Xournal++ (`.xopp`) import. It is the
same library the Android app uses (`crates/rnote-library`), so both read and write the same
`.rnote-library/library.json`.

It installs as its own Flatpak, `io.github.yx14ng.rnote`, shown as **Rnote (YX14ng)**, beside
Flathub's Rnote (`com.github.flxzt.rnote`), which stays as it is.

Its icon is the Android app's (an open notebook with a yellow stroke and a pencil, made with
ChatGPT's image generation through Codex, GPL-3.0-or-later like the rest), so the two apps
don't look alike in the app grid. The symbolic icon is traced from it with potrace; the
development build (`.Devel`) keeps upstream's icon.

**Don't open notebooks saved by the fork (or by the Android app) in Rnote 0.14.2.** The fork
is built on upstream's development branch, which saves the newer `.rnote` format 0.15
(shapes, images, PDF pages and text keep their position as an `affine` transform). Rnote
0.14.2 reads that file without an error but places all of those at the top left corner, and
saving there makes it permanent. The other way is fine: the fork opens notebooks saved by
0.14.2.

## Install

```bash
RNOTE_LIBRARY_DIR=/path/to/your/notes build-aux/install-fork-flatpak.sh
build-aux/copy-official-settings.sh   # optional: the official app's preset, pens, colors
```

`install-fork-flatpak.sh` adds Flathub as a user remote if needed, installs
`org.flatpak.Builder`, builds `build-aux/io.github.yx14ng.rnote.json` (it downloads the
GNOME 50 SDK and the Rust extension the first time, several GB, for the current user) and
installs the app for the current user. With `RNOTE_LIBRARY_DIR`, it gives the app access to
that folder and makes it the library.

`copy-official-settings.sh` copies every setting both apps have from the official Rnote to
the fork. It only reads the official app's settings. Run it again to copy them again.

Run the same commands to update after pulling.

## The library

- Without `RNOTE_LIBRARY_DIR`, the library is `Rnote` in the documents folder, created on
  first start. "Library Folder…" in the library menu picks another one; a folder outside
  the documents, desktop and pictures folders also needs
  `flatpak override --user --filesystem=/that/folder io.github.yx14ng.rnote`.
- A library folder that is missing (an unmounted disk, say) shows an error instead of an
  empty library; nothing is created in its place.
- Folders are real folders, one level deep. Tags and the trash live in `.rnote-library/`.
  Renaming a file outside the app loses its tags.
- `.xopp` notebooks show as "Xournal++ · import": opening one converts it to an `.rnote`
  beside it and leaves the `.xopp` alone. Opening it again asks before importing twice.
- Renaming or moving a notebook that is open in a tab keeps the tab saving to its new
  place. Sending an open notebook (or its folder) to the trash is refused until its tab is
  closed.
- Opening a file directly (`rnote file.rnote`, the file manager, "Open") goes straight to
  the editor, as before. The house button in the editor goes back to the library; the
  tabs stay open.
- "New" opens the new notebook dialog, as on Android: title, folder, page size and
  orientation, sheets, continuous or infinite, paper (white, cream, light gray or
  StarNote's black), template and spacing, and cover. What it chose stays for the next
  notebook of that folder (kept outside the library, never synced).
- Each notebook's menu sets its cover color, the same eight as on Android.
- The import button brings in `.rnote` and `.xopp` notebooks as they are, a PDF as a new
  notebook (password, pages, vector or images, with progress and Cancel) or an image as a
  new notebook. Dropping those files on the library does the same, into the folder shown.
- Dragging a notebook's card onto a folder of the sidebar, or onto "Uncategorized", moves
  it there.
- Sending notebooks to the trash shows a notice with Undo.
- Keyboard: Ctrl+F searches, F2 renames the notebook in focus, Delete sends it to the trash
  (the selected ones in selection mode), Ctrl+A selects all in selection mode, Escape
  leaves it and F5 reads the folder again.
- The library follows its folder: notebooks and folders that change on disk (a sync,
  another program) show up without refreshing.

## The editor

- Holding the pen still for a second at the end of a brush stroke turns it into the shape
  it looks like: a line, rectangle, ellipse, triangle, polygon, star, arrow, polyline, or a
  tall bracket like the parentheses around a matrix, made symmetric. It takes the brush's
  color and width, and undoing brings back the stroke as written. Lifting the pen without the
  pause keeps the stroke as drawn, and small strokes (handwriting) are never touched.
  "Recognize Shapes" in the settings turns it off. It is the same recognizer as on Android.

- The sidebar's "Pages" tab shows the sheets of a notebook of fixed-size pages: click a
  page to go to it, and its menu inserts a blank page or a PDF after it, duplicates,
  moves it up or down, or removes it (asking first if something is written on it), and
  "Remove Pages…" removes several at once, from one page to another. Each of those is one
  step to undo.
- Exporting a document to PDF can include the blank pages, so page numbers stay the same.
- "About" shows the fork's version and sends issues to this repository.

## Google Drive sync

The library syncs with a folder named `Rnote` in the Google Drive of whoever signs in, the
same folder the Android app syncs, so the notebooks are the same on every device. The app
only gets access to the files it creates (`drive.file`), nothing else of the Drive. See
[privacy.md](privacy.md).

- The cloud button in the library's header bar signs in: Google's page opens in the
  browser, and once you agree the button turns into a menu with the last sync, "Sync Now"
  and "Sign Out of Google Drive". Signing out tells Google to forget the app's access and
  removes the token from this computer.
- It syncs when the app opens, when you go back to the library, a few seconds after saving
  a notebook of the library or renaming, moving, tagging or trashing one, every five minutes
  while signed in, and with "Sync Now". Without a connection it waits until the connection
  is back. Closing the window finishes a sync about to start or under way first, for up to
  a minute.
- A notebook changed on two devices is never overwritten: both versions stay, one named
  `Name (conflicto, <device>, <date>).rnote`. A notebook open in a tab that a sync updates
  reloads by itself if it has no unsaved changes; with unsaved changes, saving puts them in
  a conflict copy and the tab switches to it.
- The refresh token is kept in the desktop keyring (Secret Service: GNOME Keyring, KWallet),
  which is why the Flatpak may talk to `org.freedesktop.secrets`.

### Building with the sync

The sync needs the "Desktop app" OAuth client of the Google Cloud project at build time.
`install-fork-flatpak.sh` reads it from `RNOTE_GDRIVE_CLIENT_ID` and
`RNOTE_GDRIVE_CLIENT_SECRET`, or else from the client JSON Google downloads, at
`RNOTE_GDRIVE_CLIENT_JSON` (default `~/.config/rnote-android/desktop-client.json`). It puts
them in a copy of the manifest in a private temporary folder; they never go into a tracked
file. A build without them (a fork, a development build) shows the button disabled, with a
tooltip that says why.

For a `meson` development build, export the two variables before `meson compile`.

### Release bundle

```bash
build-aux/install-fork-flatpak.sh --bundle rnote-yx14ng-desktop.flatpak
```

builds the same app into a single `.flatpak` file for a GitHub release and installs nothing.
It refuses to build without the OAuth client, since a release is for other people. The
client secret ends up inside the app, as in any distributed desktop app (Google doesn't treat
it as confidential for "Desktop app" clients), but never in the repository.

Whoever downloads it installs it with:

```bash
flatpak install --user rnote-yx14ng-desktop.flatpak
```

which also fetches the GNOME runtime from Flathub.

## Developing

`build-aux/dev-setup.sh` configures `_mesonbuild` with the development id
`io.github.yx14ng.rnote.Devel` and generates `crates/rnote-ui/src/config.rs`, which
`cargo test -p rnote` needs.
