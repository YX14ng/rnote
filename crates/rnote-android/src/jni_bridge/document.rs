//! Entry points that open, render, save and close documents.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JDoubleArray, JObject, JString};
use jni::sys::{jdouble, jint, jlong};

use super::common::{
    BridgeError, document_from_handle, edit, edit_flags, into_handle, layout_to_int,
};
use crate::android_bitmap::LockedBitmap;
use crate::android_color::color_to_argb;
use crate::{RnoteDocument, Viewport};

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

/// `static native double[] documentSettings(long handle)`, as `[pageWidthMm, pageHeightMm,
/// layout, backgroundArgb, pattern, patternArgb, showBorders, patternSize]`; the ints are
/// stored as doubles.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_documentSettings<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> JDoubleArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JDoubleArray<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let settings = document.document_settings();
            let values = [
                settings.page_width_mm,
                settings.page_height_mm,
                f64::from(layout_to_int(settings.layout)),
                f64::from(color_to_argb(settings.background_color)),
                f64::from(settings.pattern as i32),
                f64::from(color_to_argb(settings.pattern_color)),
                f64::from(u8::from(settings.show_borders)),
                settings.pattern_size,
            ];
            let array = env.new_double_array(values.len())?;
            array.set_region(env, 0, &values)?;
            Ok(array)
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
