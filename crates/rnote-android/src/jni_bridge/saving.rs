//! Entry points that save the open notebook without holding up its thread.
//!
//! A save handle is a pointer to a boxed [`PendingSave`], stored in a Java `long`. The
//! Kotlin `DocumentRenderer` owns it: every handle comes from `saveBegin` and is passed to
//! `saveWait` exactly once, after any `saveEditsOf`, never used afterwards and never from
//! two threads at once.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jlong;

use super::common::{BridgeError, document_from_handle};
use crate::rnote_document::PendingSave;

/// Resolves a save handle.
///
/// # Safety
///
/// `handle` must be 0 or a handle from `saveBegin` not passed to `saveWait` yet, used by no
/// other call at the same time (see the module docs).
unsafe fn pending_from_handle<'a>(handle: jlong) -> Result<&'a PendingSave, BridgeError> {
    // SAFETY: Non-zero handles point to a live, boxed `PendingSave` that nothing else
    // accesses during this call (caller contract).
    unsafe { (handle as *const PendingSave).as_ref() }
        .ok_or_else(|| BridgeError::Rnote("The save is over".to_owned()))
}

/// `static native long saveBegin(long document, String fileName)`: takes the document as it
/// is now to save it; called on the document's thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_saveBegin<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    document: jlong,
    file_name: JString<'caller>,
) -> jlong {
    unowned_env
        .with_env(|env| -> Result<jlong, BridgeError> {
            // SAFETY: `document` comes from `NativeDocument`, which upholds the invariants.
            let document = unsafe { document_from_handle(document) }?;
            let file_name = file_name.try_to_string(env)?;
            Ok(Box::into_raw(Box::new(document.begin_save(&file_name))) as jlong)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native long saveEditsOf(long save)`: the changes the document had when the save
/// began, for `markSavedAsOf`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_saveEditsOf<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    save: jlong,
) -> jlong {
    unowned_env
        .with_env(|_env| -> Result<jlong, BridgeError> {
            // SAFETY: `save` comes from `DocumentRenderer`, which upholds the module invariants.
            let pending = unsafe { pending_from_handle(save) }?;
            Ok(pending.edits() as jlong)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native byte[] saveWait(long save)`: the `.rnote` bytes, once made; any thread.
/// The handle is done afterwards.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_saveWait<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    save: jlong,
) -> JByteArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JByteArray<'caller>, BridgeError> {
            let pending = save as *mut PendingSave;
            if pending.is_null() {
                return Err(BridgeError::Rnote("The save is over".to_owned()));
            }
            // SAFETY: A non-zero handle owns its box, given up here (module contract).
            let pending = unsafe { Box::from_raw(pending) };
            let bytes = pending.wait()?;
            Ok(env.byte_array_from_slice(&bytes)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void markSavedAsOf(long document, long edits)`: marks the document as
/// saved unless it changed since the save that saw `edits` changes began.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_markSavedAsOf<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    document: jlong,
    edits: jlong,
) {
    unowned_env
        .with_env(|_env| -> Result<(), BridgeError> {
            // SAFETY: `document` comes from `NativeDocument`, which upholds the invariants.
            let document = unsafe { document_from_handle(document) }?;
            document.mark_saved_as_of(edits as u64);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
