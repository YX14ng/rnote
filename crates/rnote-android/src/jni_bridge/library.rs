//! Entry points of the notebook library.

use std::path::Path;

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::jint;

use super::common::BridgeError;
use crate::library_bridge::handle_request;

/// `static native String libraryCall(String root, String request)`: runs a JSON request on
/// the library at `root` and returns the JSON answer; throws with the error's message.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_libraryCall<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    root: JString<'caller>,
    request: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let root = root.try_to_string(env)?;
            let request = request.try_to_string(env)?;
            let answer = handle_request(Path::new(&root), &request)?;
            Ok(env.new_string(answer)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native byte[] libraryThumbnail(byte[] rnoteBytes, int sizePx)`: a PNG of the
/// notebook's first page, or null if it can't be rendered.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_libraryThumbnail<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    rnote_bytes: JByteArray<'caller>,
    size_px: jint,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let bytes = env.convert_byte_array(&rnote_bytes)?;
            let size = u32::try_from(size_px).unwrap_or(0).max(1);
            match rnote_library::render_thumbnail(bytes, size) {
                Ok(png) => Ok(env.byte_array_from_slice(&png)?.into()),
                Err(_) => Ok(JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
