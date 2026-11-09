//! Entry points that change brush, shape, text and document settings.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jdouble, jint, jlong};
use rnote_compose::builders::ShapeBuilderType;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::pens::pensconfig::brushconfig::BrushStyle;
use rnote_engine::pens::pensconfig::eraserconfig::EraserStyle;
use rnote_engine::strokes::textstroke::TextAlignment;

use super::common::{BridgeError, document_from_handle, edit, enum_from_int, layout_from_int};
use crate::android_color::color_from_argb;
use crate::{ShapeStyle, TextFormat};

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

/// `static native int setTextFormat(long handle, String fontFamily, double fontSize,
/// boolean bold, boolean italic, int alignment)`; alignment 0 start, 1 center, 2 end, 3 fill.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setTextFormat<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    font_family: JString<'caller>,
    font_size: jdouble,
    bold: jboolean,
    italic: jboolean,
    alignment: jint,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let alignment = match alignment {
                0 => TextAlignment::Start,
                1 => TextAlignment::Center,
                2 => TextAlignment::End,
                3 => TextAlignment::Fill,
                other => {
                    return Err(BridgeError::Rnote(format!("Unknown text alignment {other}")));
                }
            };
            let format = TextFormat {
                font_family: font_family.try_to_string(env)?,
                font_size,
                bold,
                italic,
                alignment,
            };
            Ok(super::common::edit_flags(document.set_text_format(&format)))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int setShapeStyle(long handle, boolean rough, boolean filled, int fillArgb)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setShapeStyle<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    rough: jboolean,
    filled: jboolean,
    fill_argb: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        let fill = filled.then(|| color_from_argb(fill_argb));
        document.set_shape_style(ShapeStyle { rough, fill });
        Ok(document.edit_state())
    })
}

/// `static native int setPageSize(long handle, double widthMm, double heightMm)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setPageSize<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    width_mm: jdouble,
    height_mm: jdouble,
) -> jint {
    edit(&mut unowned_env, handle, |document| Ok(document.set_page_size(width_mm, height_mm)))
}

/// `static native int setLayout(long handle, int layout)`, see `layout_from_int`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setLayout<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    layout: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| Ok(document.set_layout(layout_from_int(layout)?)))
}

/// `static native int setBackground(long handle, int argb, int pattern, int patternArgb,
/// double patternSize)`; `pattern` is a `PatternStyle` discriminant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setBackground<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    argb: jint,
    pattern: jint,
    pattern_argb: jint,
    pattern_size: jdouble,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        let pattern = enum_from_int::<PatternStyle>(pattern, "pattern")?;
        Ok(document.set_background(
            color_from_argb(argb),
            pattern,
            color_from_argb(pattern_argb),
            pattern_size,
        ))
    })
}

/// `static native int setPageBorders(long handle, boolean show)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_setPageBorders<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    show: jboolean,
) -> jint {
    edit(&mut unowned_env, handle, |document| Ok(document.set_page_borders(show)))
}
