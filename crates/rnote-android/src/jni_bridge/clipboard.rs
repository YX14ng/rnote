//! Entry points for copying, cutting and pasting.
//!
//! Copied content travels as a `String` of the form `"<mime type>\n<data>"`: Rnote stroke
//! content (JSON) or plain text, the formats a document can paste back.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jdouble, jint, jlong};
use parry2d_f64::math::Vector2;

use super::common::{BridgeError, document_from_handle, edit_flags};
use crate::ClipboardEntry;

/// Encodes an entry as `"<mime type>\n<data>"`. Both formats offered are UTF-8 text.
fn encode(entry: ClipboardEntry) -> Result<String, BridgeError> {
    let data = String::from_utf8(entry.data)
        .map_err(|err| BridgeError::Rnote(format!("Copied data is not text: {err}")))?;
    Ok(format!("{}\n{data}", entry.mime_type))
}

/// `static native String copy(long handle, boolean cut)`, or null when nothing is selected.
/// With `cut`, what was copied is removed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_copy<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    cut: jni::sys::jboolean,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let entry = if cut { document.cut()?.0 } else { document.copy()? };
            match entry {
                Some(entry) => Ok(env.new_string(encode(entry)?)?.into()),
                None => Ok(JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int pasteStrokeContent(long handle, String json, double x, double y)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pasteStrokeContent<
    'caller,
>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    json: JString<'caller>,
    x: jdouble,
    y: jdouble,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let json = json.try_to_string(env)?;
            let state = document.paste_stroke_content(&json, Vector2::new(x, y))?;
            Ok(edit_flags(state))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int pasteText(long handle, String text, double x, double y)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pasteText<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    text: JString<'caller>,
    x: jdouble,
    y: jdouble,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let text = text.try_to_string(env)?;
            Ok(edit_flags(document.paste_text(&text, Vector2::new(x, y))))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int pasteImage(long handle, byte[] bytes, double x, double y)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pasteImage<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    bytes: JByteArray<'caller>,
    x: jdouble,
    y: jdouble,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let bytes = env.convert_byte_array(&bytes)?;
            let state = document.paste_image(bytes, Vector2::new(x, y))?;
            Ok(edit_flags(state))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_mime_type_and_data() {
        let entry = ClipboardEntry { mime_type: "text/plain".to_owned(), data: b"a\nb".to_vec() };

        assert_eq!(encode(entry).ok().as_deref(), Some("text/plain\na\nb"));
    }
}
