//! Bridge between `rnote-engine` and the Android viewer app.
//!
//! [`DocumentView`] is plain Rust and is tested on the host. The JNI entry points and the
//! access to Android bitmaps only exist when targeting Android.

#[cfg(target_os = "android")]
mod android_bitmap;
mod document_view;
#[cfg(target_os = "android")]
mod jni_bridge;

pub use document_view::{BYTES_PER_PIXEL, DESK_COLOR, DocumentView, PixelLayout, Viewport};
