//! Entry points of the Drive sync and of saving library notebooks.

use std::path::Path;

use chrono::Local;
use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use rnote_sync::DriveClient;
use serde_json::json;

use super::common::BridgeError;
use crate::sync_bridge::{run_sync, write_notebook};

/// `static native String syncRun(String root, String statePath, String device, String
/// token)`: syncs the library at `root` with Drive, with the access `token`, and returns
/// the JSON outcome. Blocks on the network: never on the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_syncRun<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    root: JString<'caller>,
    state_path: JString<'caller>,
    device: JString<'caller>,
    token: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let root = root.try_to_string(env)?;
            let state_path = state_path.try_to_string(env)?;
            let device = device.try_to_string(env)?;
            let token = token.try_to_string(env)?;
            let mut drive = DriveClient::new(move || Ok(token.clone()));
            let now = Local::now().naive_local();
            let answer =
                run_sync(Path::new(&root), Path::new(&state_path), &device, &mut drive, now);
            Ok(env.new_string(answer)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void syncRevoke(String token)`: signs out of Google Drive, revoking the
/// access `token`; throws if Google can't be told. Blocks on the network.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_syncRevoke<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    token: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> Result<(), BridgeError> {
            let token = token.try_to_string(env)?;
            rnote_sync::revoke(&token).map_err(anyhow::Error::from)?;
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `static native String libraryWriteNotebook(String root, String path, String
/// expectedSha256, String device, byte[] bytes)`: saves the open notebook to `path`, or to
/// a conflict copy beside it if the file changed since the editor read it; returns
/// `{"path", "sha256", "conflict"}`. `expectedSha256` may be null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_libraryWriteNotebook<
    'caller,
>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    root: JString<'caller>,
    path: JString<'caller>,
    expected_sha256: JString<'caller>,
    device: JString<'caller>,
    bytes: JByteArray<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            let root = root.try_to_string(env)?;
            let path = path.try_to_string(env)?;
            let expected = if expected_sha256.is_null() {
                None
            } else {
                Some(expected_sha256.try_to_string(env)?)
            };
            let device = device.try_to_string(env)?;
            let bytes = env.convert_byte_array(&bytes)?;
            let now = Local::now().naive_local();
            let written =
                write_notebook(Path::new(&root), &path, expected.as_deref(), &device, now, &bytes)?;
            let answer = json!({
                "path": written.path,
                "sha256": written.sha256,
                "conflict": written.conflict,
            });
            Ok(env.new_string(answer.to_string())?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
