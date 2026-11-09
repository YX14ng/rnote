//! JNI entry points for `io.github.yx14ng.rnote.viewer.NativeBridge`.
//!
//! A document handle is a pointer to a boxed [`RnoteDocument`], stored in a Java `long`.
//! The Kotlin `NativeDocument` class owns it and upholds these invariants:
//!
//! - every handle comes from `open` or `newDocument` and is passed to `close` exactly once;
//! - a handle is never used after `close`;
//! - calls for one handle never overlap (they are serialized on a single thread).
//!
//! Operations that change the document return an [`EditState`] packed by `edit_flags` in `common`.

mod common;
mod document;
mod editing;
mod settings;
