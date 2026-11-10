# Rnote (YX14ng) on the desktop

The fork's desktop build adds a **library** home screen to Rnote: the notebooks of one
folder, with folders, tags, a trash, thumbnails and Xournal++ (`.xopp`) import. It is the
same library the Android app uses (`crates/rnote-library`), so both read and write the same
`.rnote-library/library.json`.

It installs as its own Flatpak, `io.github.yx14ng.rnote`, shown as **Rnote (YX14ng)**, beside
Flathub's Rnote (`com.github.flxzt.rnote`), which stays as it is. Files stay compatible:
the official Rnote opens the fork's notebooks.

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

## Developing

`build-aux/dev-setup.sh` configures `_mesonbuild` with the development id
`io.github.yx14ng.rnote.Devel` and generates `crates/rnote-ui/src/config.rs`, which
`cargo test -p rnote` needs.
