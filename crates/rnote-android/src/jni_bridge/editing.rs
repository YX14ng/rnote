//! Entry points for pen input, keys, tools, selection and history.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JDoubleArray, JString};
use jni::sys::{jboolean, jdouble, jint, jlong};

use super::common::{
    BridgeError, VALUES_PER_SAMPLE, document_from_handle, edit, edit_flags, key_from_int,
    tool_from_int,
};
use crate::{PenPhase, PenSample, ViewSize, Viewport};

/// `static native int handlePen(long handle, int phase, int tool, double[] samples,
/// double x, double y, double zoom, int viewWidth, int viewHeight)`
///
/// `phase` is 0 (down), 1 (up), 2 (cancel) or 3 (proximity); `tool` as in `tool_from_int`.
/// `samples` holds `x, y, pressure` triples in document coordinates.
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
                3 => PenPhase::Proximity,
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

/// `static native int handleKey(long handle, int key, boolean shift, boolean ctrl)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_handleKey<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    key: jint,
    shift: jboolean,
    ctrl: jboolean,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        Ok(document.handle_key(key_from_int(key)?, shift, ctrl))
    })
}

/// `static native int handleText(long handle, String text)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_handleText<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    text: JString<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let text = text.try_to_string(env)?;
            Ok(edit_flags(document.handle_text(&text)))
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
