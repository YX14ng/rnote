//! Helpers shared by the JNI entry points: errors, handles and argument decoding.

use std::fmt;

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::sys::{jint, jlong};
use num_traits::FromPrimitive;
use rnote_compose::penevent::KeyboardKey;

use crate::{EditState, RnoteDocument, Tool};

/// Bits of the `int` returned by editing calls. Mirrored in `NativeDocument.kt`.
const FLAG_MODIFIED: jint = 1;
const FLAG_CAN_UNDO: jint = 1 << 1;
const FLAG_CAN_REDO: jint = 1 << 2;
const FLAG_BOUNDS_CHANGED: jint = 1 << 3;
const FLAG_HAS_SELECTION: jint = 1 << 4;
const FLAG_TEXT_INPUT: jint = 1 << 5;

/// Values per sample in the `samples` array of `handlePen`: x, y and pressure.
pub(super) const VALUES_PER_SAMPLE: usize = 3;

/// Errors surfaced to Java as a `RuntimeException`.
#[derive(Debug)]
pub(super) enum BridgeError {
    Jni(jni::errors::Error),
    Rnote(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jni(err) => write!(f, "JNI error: {err}"),
            Self::Rnote(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<jni::errors::Error> for BridgeError {
    fn from(err: jni::errors::Error) -> Self {
        Self::Jni(err)
    }
}

impl From<anyhow::Error> for BridgeError {
    fn from(err: anyhow::Error) -> Self {
        // `{:#}` keeps the whole context chain in a single line.
        Self::Rnote(format!("{err:#}"))
    }
}

pub(super) fn edit_flags(state: EditState) -> jint {
    let mut flags = 0;
    for (set, flag) in [
        (state.modified, FLAG_MODIFIED),
        (state.can_undo, FLAG_CAN_UNDO),
        (state.can_redo, FLAG_CAN_REDO),
        (state.bounds_changed, FLAG_BOUNDS_CHANGED),
        (state.has_selection, FLAG_HAS_SELECTION),
        (state.text_input, FLAG_TEXT_INPUT),
    ] {
        if set {
            flags |= flag;
        }
    }
    flags
}

/// Decodes a tool: 0 brush, 1 eraser, 2 shaper, 3 selector, 4 typewriter. Mirrored in
/// `NativeDocument.kt`.
pub(super) fn tool_from_int(tool: jint) -> Result<Tool, BridgeError> {
    match tool {
        0 => Ok(Tool::Brush),
        1 => Ok(Tool::Eraser),
        2 => Ok(Tool::Shaper),
        3 => Ok(Tool::Selector),
        4 => Ok(Tool::Typewriter),
        other => Err(BridgeError::Rnote(format!("Unknown tool {other}"))),
    }
}

/// Decodes an editing key, as numbered by `EditingKey` in `NativeDocument.kt`.
pub(super) fn key_from_int(key: jint) -> Result<KeyboardKey, BridgeError> {
    Ok(match key {
        0 => KeyboardKey::BackSpace,
        1 => KeyboardKey::Delete,
        2 => KeyboardKey::CarriageReturn,
        3 => KeyboardKey::Escape,
        4 => KeyboardKey::NavLeft,
        5 => KeyboardKey::NavRight,
        6 => KeyboardKey::NavUp,
        7 => KeyboardKey::NavDown,
        8 => KeyboardKey::Home,
        9 => KeyboardKey::End,
        10 => KeyboardKey::HorizontalTab,
        other => return Err(BridgeError::Rnote(format!("Unknown key {other}"))),
    })
}

/// Decodes one of the engine's enums from its discriminant, as the Kotlin side sends them.
pub(super) fn enum_from_int<T: FromPrimitive>(value: jint, what: &str) -> Result<T, BridgeError> {
    T::from_i32(value).ok_or_else(|| BridgeError::Rnote(format!("Unknown {what} {value}")))
}

/// Runs `operation` on the document behind `handle` and returns the packed edit state.
pub(super) fn edit(
    unowned_env: &mut EnvUnowned<'_>,
    handle: jlong,
    operation: impl FnOnce(&mut RnoteDocument) -> Result<EditState, BridgeError>,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(edit_flags(operation(document)?))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

pub(super) fn into_handle(document: RnoteDocument) -> jlong {
    Box::into_raw(Box::new(document)) as jlong
}

/// Resolves a handle to the document it points to.
///
/// # Safety
///
/// `handle` must be 0 or a handle returned by `open` or `newDocument` that has not been
/// closed, and no other call may use it concurrently (see the module docs).
pub(super) unsafe fn document_from_handle<'a>(
    handle: jlong,
) -> Result<&'a mut RnoteDocument, BridgeError> {
    let document = handle as *mut RnoteDocument;
    // SAFETY: Non-zero handles point to a live, boxed `RnoteDocument` that nothing else
    // accesses during this call (caller contract).
    unsafe { document.as_mut() }.ok_or_else(|| BridgeError::Rnote("Document is closed".to_owned()))
}
