//! Bridge between `rnote-engine` and the Android app.
//!
//! [`RnoteDocument`] is plain Rust and is tested on the host. The JNI entry points and the
//! access to Android bitmaps only exist when targeting Android.

#[cfg(target_os = "android")]
mod android_bitmap;
mod android_color;
mod document_settings;
#[cfg(target_os = "android")]
mod jni_bridge;
mod pen_input;
mod render;
mod rnote_document;
mod shape_recognition;

pub use android_color::{color_from_argb, color_to_argb};
pub use document_settings::{DocumentSettings, ShapeStyle, TextFormat};
pub use pen_input::{PenPhase, PenSample, Tool};
pub use render::{BYTES_PER_PIXEL, DESK_COLOR, PixelLayout, Viewport};
pub use rnote_document::{ClipboardEntry, EditState, RnoteDocument, TextEmphasis, ViewSize};
