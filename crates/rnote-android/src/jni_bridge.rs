//! JNI entry points for `io.github.yx14ng.rnote.viewer.NativeBridge`.
//!
//! A document handle is a pointer to a boxed [`RnoteDocument`], stored in a Java `long`.
//! The Kotlin `NativeDocument` class owns it and upholds these invariants:
//!
//! - every handle comes from `open` or `newDocument` and is passed to `close` exactly once;
//! - a handle is never used after `close`;
//! - calls for one handle never overlap (they are serialized on a single thread).
//!
//! Operations that change the document return an [`EditState`] packed by [`edit_flags`].

use std::fmt;

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JDoubleArray, JObject, JString};
use jni::sys::{jdouble, jint, jlong};
use num_traits::FromPrimitive;
use rnote_compose::builders::ShapeBuilderType;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::pens::pensconfig::eraserconfig::EraserStyle;

use crate::android_bitmap::LockedBitmap;
use crate::android_color::{color_from_argb, color_to_argb};
use crate::{EditState, PenPhase, PenSample, RnoteDocument, Tool, ViewSize, Viewport};

/// Bits of the `int` returned by editing calls. Mirrored in `NativeDocument.kt`.
const FLAG_MODIFIED: jint = 1;
const FLAG_CAN_UNDO: jint = 1 << 1;
const FLAG_CAN_REDO: jint = 1 << 2;
const FLAG_BOUNDS_CHANGED: jint = 1 << 3;
const FLAG_HAS_SELECTION: jint = 1 << 4;

/// Values per sample in the `samples` array of `handlePen`: x, y and pressure.
const VALUES_PER_SAMPLE: usize = 3;

/// Errors surfaced to Java as a `RuntimeException`.
#[derive(Debug)]
enum BridgeError {
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

fn edit_flags(state: EditState) -> jint {
    let mut flags = 0;
    for (set, flag) in [
        (state.modified, FLAG_MODIFIED),
        (state.can_undo, FLAG_CAN_UNDO),
        (state.can_redo, FLAG_CAN_REDO),
        (state.bounds_changed, FLAG_BOUNDS_CHANGED),
        (state.has_selection, FLAG_HAS_SELECTION),
    ] {
        if set {
            flags |= flag;
        }
    }
    flags
}

/// Decodes a tool: 0 brush, 1 eraser, 2 shaper, 3 selector. Mirrored in `NativeDocument.kt`.
fn tool_from_int(tool: jint) -> Result<Tool, BridgeError> {
    match tool {
        0 => Ok(Tool::Brush),
        1 => Ok(Tool::Eraser),
        2 => Ok(Tool::Shaper),
        3 => Ok(Tool::Selector),
        other => Err(BridgeError::Rnote(format!("Unknown tool {other}"))),
    }
}

/// Decodes one of the engine's enums from its discriminant, as the Kotlin side sends them.
fn enum_from_int<T: FromPrimitive>(value: jint, what: &str) -> Result<T, BridgeError> {
    T::from_i32(value).ok_or_else(|| BridgeError::Rnote(format!("Unknown {what} {value}")))
}

/// Runs `operation` on the document behind `handle` and returns the packed edit state.
fn edit(
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

fn into_handle(document: RnoteDocument) -> jlong {
    Box::into_raw(Box::new(document)) as jlong
}

/// Resolves a handle to the document it points to.
///
/// # Safety
///
/// `handle` must be 0 or a handle returned by `open` or `newDocument` that has not been
/// closed, and no other call may use it concurrently (see the module docs).
unsafe fn document_from_handle<'a>(handle: jlong) -> Result<&'a mut RnoteDocument, BridgeError> {
    let document = handle as *mut RnoteDocument;
    // SAFETY: Non-zero handles point to a live, boxed `RnoteDocument` that nothing else
    // accesses during this call (caller contract).
    unsafe { document.as_mut() }.ok_or_else(|| BridgeError::Rnote("Document is closed".to_owned()))
}

/// `static native long open(byte[] rnoteBytes)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_open<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    rnote_bytes: JByteArray<'caller>,
) -> jlong {
    unowned_env
        .with_env(|env| -> Result<jlong, BridgeError> {
            let bytes = env.convert_byte_array(&rnote_bytes)?;
            Ok(into_handle(RnoteDocument::try_open(bytes)?))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native long newDocument()`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_newDocument<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> jlong {
    unowned_env
        .with_env(|_env| -> Result<jlong, BridgeError> {
            Ok(into_handle(RnoteDocument::new_empty()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native double[] bounds(long handle)`, as `[minX, minY, maxX, maxY]`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_bounds<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> JDoubleArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JDoubleArray<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let bounds = document.bounds();
            let array = env.new_double_array(4)?;
            array.set_region(
                env,
                0,
                &[bounds.mins.x, bounds.mins.y, bounds.maxs.x, bounds.maxs.y],
            )?;
            Ok(array)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int backgroundColor(long handle)`, as an `android.graphics.Color` int.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_backgroundColor<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(color_to_argb(document.background_color()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int editState(long handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_editState<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(edit_flags(document.edit_state()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void render(long handle, Bitmap target, double x, double y, double zoom)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_render<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    target: JObject<'caller>,
    x: jdouble,
    y: jdouble,
    zoom: jdouble,
) {
    unowned_env
        .with_env(|env| -> Result<(), BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            // SAFETY: `NativeDocument.render` passes a bitmap it allocated for this call and
            // only hands it to the UI after this function returns, so nothing else touches
            // its pixels while they are locked.
            let mut bitmap = unsafe { LockedBitmap::lock(env, &target) }?;
            let (pixels, layout) = bitmap.pixels_mut();
            document.render_rgba(Viewport { x, y, zoom }, layout, pixels)?;
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int handlePen(long handle, int phase, int tool, double[] samples,
/// double x, double y, double zoom, int viewWidth, int viewHeight)`
///
/// `phase` is 0 (down), 1 (up) or 2 (cancel); `tool` is 0 (brush) or 1 (eraser). `samples`
/// holds `x, y, pressure` triples in document coordinates.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_handlePen<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    phase: jint,
    tool: jint,
    samples: JDoubleArray<'caller>,
    x: jdouble,
    y: jdouble,
    zoom: jdouble,
    view_width: jint,
    view_height: jint,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let phase = match phase {
                0 => PenPhase::Down,
                1 => PenPhase::Up,
                2 => PenPhase::Cancel,
                other => return Err(BridgeError::Rnote(format!("Unknown pen phase {other}"))),
            };
            let tool = tool_from_int(tool)?;
            let mut values = vec![0.0; samples.len(env)?];
            samples.get_region(env, 0, &mut values)?;
            let (triples, rest) = values.as_chunks::<VALUES_PER_SAMPLE>();
            if !rest.is_empty() {
                return Err(BridgeError::Rnote(format!(
                    "Sample array length {} is not a multiple of {VALUES_PER_SAMPLE}",
                    values.len()
                )));
            }
            let samples: Vec<PenSample> =
                triples.iter().map(|&[x, y, pressure]| PenSample { x, y, pressure }).collect();
            let view_size = ViewSize {
                width: u32::try_from(view_width).unwrap_or(0),
                height: u32::try_from(view_height).unwrap_or(0),
            };
            let state =
                document.handle_pen(phase, tool, &samples, Viewport { x, y, zoom }, view_size);
            Ok(edit_flags(state))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int undo(long handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_undo<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(edit_flags(document.undo()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int redo(long handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_redo<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(edit_flags(document.redo()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void setBrush(long handle, int argb, double width)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setBrush<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    argb: jint,
    width: jdouble,
) {
    unowned_env
        .with_env(|_env| -> Result<(), BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            document.set_brush(color_from_argb(argb), width);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native byte[] save(long handle, String fileName)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_save<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    file_name: JString<'caller>,
) -> JByteArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JByteArray<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let file_name = file_name.try_to_string(env)?;
            let bytes = document.save_to_bytes(&file_name)?;
            Ok(env.byte_array_from_slice(&bytes)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int setTool(long handle, int tool)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setTool<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    tool: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| Ok(document.set_tool(tool_from_int(tool)?)))
}

/// `static native int setShape(long handle, int shape)`, a `ShapeBuilderType` discriminant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setShape<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    shape: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        document.set_shape(enum_from_int::<ShapeBuilderType>(shape, "shape")?);
        Ok(document.edit_state())
    })
}

/// `static native int setBrushStyle(long handle, int style)`, a `BrushStyle` discriminant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setBrushStyle<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    style: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        document.set_brush_style(enum_from_int::<BrushStyle>(style, "brush style")?);
        Ok(document.edit_state())
    })
}

/// `static native int setEraserStyle(long handle, int style)`, an `EraserStyle` discriminant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setEraserStyle<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    style: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        document.set_eraser_style(enum_from_int::<EraserStyle>(style, "eraser style")?);
        Ok(document.edit_state())
    })
}

/// `static native int selection(long handle, int action)`: 0 select all, 1 deselect all,
/// 2 delete, 3 duplicate.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_selection<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    action: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| match action {
        0 => Ok(document.select_all()),
        1 => Ok(document.deselect_all()),
        2 => Ok(document.delete_selection()),
        3 => Ok(document.duplicate_selection()),
        other => Err(BridgeError::Rnote(format!("Unknown selection action {other}"))),
    })
}

/// `static native int markModified(long handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_markModified<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        document.mark_modified();
        Ok(document.edit_state())
    })
}

/// `static native void markSaved(long handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_markSaved<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) {
    unowned_env
        .with_env(|_env| -> Result<(), BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            document.mark_saved();
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void close(long handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_close<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) {
    unowned_env
        .with_env(|_env| -> Result<(), BridgeError> {
            if handle != 0 {
                // SAFETY: Non-zero handles come from `Box::into_raw` in `into_handle`, and
                // `NativeDocument` closes each one exactly once and never uses it afterwards.
                drop(unsafe { Box::from_raw(handle as *mut RnoteDocument) });
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
