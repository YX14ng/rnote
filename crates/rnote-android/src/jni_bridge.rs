//! JNI entry points for `io.github.yx14ng.rnote.viewer.NativeBridge`.
//!
//! A document handle is a pointer to a boxed [`DocumentView`], stored in a Java `long`.
//! The Kotlin `NativeDocument` class owns it and upholds these invariants:
//!
//! - every handle comes from `open` and is passed to `close` exactly once;
//! - a handle is never used after `close`;
//! - calls for one handle never overlap (they are serialized on a single thread).

use std::fmt;

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JDoubleArray, JObject};
use jni::sys::{jdouble, jlong};

use crate::android_bitmap::LockedBitmap;
use crate::{DocumentView, Viewport};

/// Errors surfaced to Java as a `RuntimeException`.
#[derive(Debug)]
enum BridgeError {
    Jni(jni::errors::Error),
    Rnote(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jni(err) => write!(f, "JNI error: {err}"),
            Self::Rnote(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<jni::errors::Error> for BridgeError {
    fn from(err: jni::errors::Error) -> Self {
        Self::Jni(err)
    }
}

impl From<anyhow::Error> for BridgeError {
    fn from(err: anyhow::Error) -> Self {
        // `{:#}` keeps the whole context chain in a single line.
        Self::Rnote(format!("{err:#}"))
    }
}

/// Resolves a handle to the document it points to.
///
/// # Safety
///
/// `handle` must be 0 or a handle returned by `open` that has not been closed, and no
/// other call may use it concurrently (see the module docs).
unsafe fn document_from_handle<'a>(handle: jlong) -> Result<&'a DocumentView, BridgeError> {
    let document = handle as *const DocumentView;
    // SAFETY: Non-zero handles point to a live, boxed `DocumentView` (caller contract).
    unsafe { document.as_ref() }.ok_or_else(|| BridgeError::Rnote("Document is closed".to_owned()))
}

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
            let document = DocumentView::try_new(bytes)?;
            Ok(Box::into_raw(Box::new(document)) as jlong)
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
                // SAFETY: Non-zero handles come from `Box::into_raw` in `open`, and
                // `NativeDocument` closes each one exactly once and never uses it afterwards.
                drop(unsafe { Box::from_raw(handle as *mut DocumentView) });
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
