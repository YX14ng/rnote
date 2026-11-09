# Superproject that cross-compiles the C libraries rnote-engine links against
# (glib/gio, cairo, pango/pangocairo, libxml2 and their transitive deps) for Android.
#
# Every dependency resolves through a pinned WrapDB wrap in subprojects/, so the
# build never picks up host libraries. Run it through android/scripts/build-deps.sh.
project('rnote-android-deps', 'c', version: '0', meson_version: '>= 1.4')

deps = [
  dependency('glib-2.0'),
  dependency('gobject-2.0'),
  dependency('gio-2.0'),
  dependency('cairo'),
  dependency('pango'),
  dependency('pangocairo'),
  # librsvg parses SVG through libxml2's SAX API.
  dependency('libxml-2.0'),
]

summary({'resolved': deps.length()}, section: 'Dependencies')
