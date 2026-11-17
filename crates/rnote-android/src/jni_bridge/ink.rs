//! Entry points of the handwriting search: the fragments the recognizer still has to read,
//! storing what it read, and searching. The index is a file per notebook in the app's cache.

use std::path::Path;

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::sys::jlong;

use super::common::{BridgeError, document_from_handle};
use crate::ink_index::store::{self, InkIndex};

/// `static native String inkPending(long handle, String indexPath, String language)`: the
/// fragments with no reading yet, as JSON (see `RnoteDocument::ink_pending_json`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_inkPending<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    index_path: JString<'caller>,
    language: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let index_path = index_path.try_to_string(env)?;
            let language = language.try_to_string(env)?;
            let json = document.ink_pending_json(Path::new(&index_path), &language)?;
            Ok(env.new_string(json)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void inkAddAll(String indexPath, String language, String readingsJson)`:
/// keeps what the recognizer read in several fragments, as JSON
/// `[{"hash": ..., "rect": [left, top, right, bottom], "candidates": [...]}]`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_inkAddAll<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    index_path: JString<'caller>,
    language: JString<'caller>,
    readings_json: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> Result<(), BridgeError> {
            let index_path = index_path.try_to_string(env)?;
            let language = language.try_to_string(env)?;
            let readings: Vec<store::Reading> =
                serde_json::from_str(&readings_json.try_to_string(env)?)
                    .map_err(anyhow::Error::from)?;
            store::add_readings(Path::new(&index_path), &language, &readings)?;
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native String inkSearch(long handle, String indexPath, String query)`: where the
/// query is, as JSON `[{"rects": [[left, top, right, bottom], ...]}]` in reading order.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_inkSearch<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    index_path: JString<'caller>,
    query: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let index_path = index_path.try_to_string(env)?;
            let query = query.try_to_string(env)?;
            let json = document.ink_search_json(Path::new(&index_path), &query)?;
            Ok(env.new_string(json)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native String inkLanguage(String indexPath, String fallback)`: the language the
/// index at `indexPath` was read in, or `fallback` without one.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_inkLanguage<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    index_path: JString<'caller>,
    fallback: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let index_path = index_path.try_to_string(env)?;
            let language = InkIndex::stored_language(Path::new(&index_path))
                .map_or_else(|| fallback.try_to_string(env), Ok)?;
            Ok(env.new_string(language)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
