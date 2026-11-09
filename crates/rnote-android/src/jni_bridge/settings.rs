//! Entry points that change brush, shape, text and document settings.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::JClass;
use jni::sys::{jdouble, jint, jlong};
use rnote_compose::builders::ShapeBuilderType;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::pens::pensconfig::eraserconfig::EraserStyle;

use super::common::{BridgeError, document_from_handle, edit, enum_from_int};
use crate::android_color::color_from_argb;

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
