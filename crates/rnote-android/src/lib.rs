//! Bridge between `rnote-engine` and the Android app.
//!
//! [`RnoteDocument`] is plain Rust and is tested on the host. The JNI entry points and the
//! access to Android bitmaps only exist when targeting Android.

#[cfg(target_os = "android")]
mod android_bitmap;
mod android_color;
mod document_settings;
mod frame_cache;
mod image_layer;
#[cfg(target_os = "android")]
mod jni_bridge;
// Only the JNI entry points, built for Android, call it outside of tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod ink_index;
mod library_bridge;
mod pdf_export;
mod pen_input;
mod render;
mod rnote_document;
mod sync_bridge;

pub use android_color::{color_from_argb, color_to_argb};
pub use document_settings::{DocumentSettings, ShapeStyle, TextFormat};
pub use pen_input::{PenPhase, PenSample, Tool};
pub use render::{BYTES_PER_PIXEL, DESK_COLOR, PixelLayout, Viewport};
pub use rnote_document::{ClipboardEntry, EditState, RnoteDocument, TextEmphasis, ViewSize};
