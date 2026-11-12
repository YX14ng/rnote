# Rnote (YX14ng) on the desktop

The fork's desktop build adds a **library** home screen to Rnote: the notebooks of one
folder, with folders, tags, a trash, thumbnails and Xournal++ (`.xopp`) import. It is the
same library the Android app uses (`crates/rnote-library`), so both read and write the same
`.rnote-library/library.json`.

It installs as its own Flatpak, `io.github.yx14ng.rnote`, shown as **Rnote (YX14ng)**, beside
Flathub's Rnote (`com.github.flxzt.rnote`), which stays as it is.

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
  a notebook of the library, and with "Sync Now". Without a connection it waits for the next
  of those.
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

## Developing

`build-aux/dev-setup.sh` configures `_mesonbuild` with the development id
`io.github.yx14ng.rnote.Devel` and generates `crates/rnote-ui/src/config.rs`, which
`cargo test -p rnote` needs.
