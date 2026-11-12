//! Entry points that make new notebooks: from a setup, from an image, and the setup last
//! used in each folder.

use std::path::Path;

use anyhow::Context;
use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};

use super::common::BridgeError;
use crate::notebook_defaults::{defaults_for, remember};
use crate::notebook_setup::{NotebookSetup, image_notebook_bytes, new_notebook_bytes};

pub(super) fn setup_from_json(json: &str) -> anyhow::Result<NotebookSetup> {
    serde_json::from_str(json).context("Reading the notebook setup")
}

/// A Java `String` that may be null.
pub(super) fn optional_string(
    env: &mut jni::Env<'_>,
    string: &JString<'_>,
) -> Result<Option<String>, BridgeError> {
    if string.is_null() { Ok(None) } else { Ok(Some(string.try_to_string(env)?)) }
}

/// `static native byte[] newNotebook(String setupJson, String fileName)`: the `.rnote`
/// bytes of an empty notebook set up as the JSON says.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_newNotebook<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    setup_json: JString<'caller>,
    file_name: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let setup = setup_from_json(&setup_json.try_to_string(env)?)?;
            let bytes = new_notebook_bytes(&setup, &file_name.try_to_string(env)?)?;
            Ok(env.byte_array_from_slice(&bytes)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native byte[] imageNotebook(String setupJson, byte[] image, String fileName)`:
/// like `newNotebook`, with the image across the top of the first page.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_imageNotebook<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    setup_json: JString<'caller>,
    image: JByteArray<'caller>,
    file_name: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let setup = setup_from_json(&setup_json.try_to_string(env)?)?;
            let image = env.convert_byte_array(&image)?;
            let bytes = image_notebook_bytes(&setup, &image, &file_name.try_to_string(env)?)?;
            Ok(env.byte_array_from_slice(&bytes)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native String notebookDefaults(String file, String folder)`: the setup JSON
/// last remembered for `folder` (null = outside any folder), or the default one.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_notebookDefaults<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    file: JString<'caller>,
    folder: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let file = file.try_to_string(env)?;
            let folder = optional_string(env, &folder)?;
            let setup = defaults_for(Path::new(&file), folder.as_deref());
            let json = serde_json::to_string(&setup).map_err(anyhow::Error::from)?;
            Ok(env.new_string(json)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void rememberNotebookDefaults(String file, String folder, String
/// setupJson)`: remembers the setup for the next notebook of `folder`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_rememberNotebookDefaults<
    'caller,
>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    file: JString<'caller>,
    folder: JString<'caller>,
    setup_json: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> Result<(), BridgeError> {
            let file = file.try_to_string(env)?;
            let folder = optional_string(env, &folder)?;
            let setup = setup_from_json(&setup_json.try_to_string(env)?)?;
            remember(Path::new(&file), folder.as_deref(), &setup)?;
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
