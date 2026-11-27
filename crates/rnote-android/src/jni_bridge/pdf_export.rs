//! Entry points that export the open notebook as a PDF one sheet at a time.
//!
//! An export handle is a pointer to a boxed [`PdfExportJob`], stored in a Java `long`. The
//! Kotlin `PdfExport` owns it: every handle comes from `pdfExportBegin` and is passed to
//! `pdfExportFinish` or `pdfExportCancel` exactly once, never used afterwards, and never
//! from two threads at once.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jint, jlong};

use super::common::{BridgeError, document_from_handle};
use crate::pdf_export::{PdfExportJob, PdfExportOptions};

/// Resolves an export handle.
///
/// # Safety
///
/// `handle` must be 0 or a handle from `pdfExportBegin` that wasn't finished or cancelled,
/// used by no other call at the same time (see the module docs).
unsafe fn job_from_handle<'a>(handle: jlong) -> Result<&'a mut PdfExportJob, BridgeError> {
    // SAFETY: Non-zero handles point to a live, boxed `PdfExportJob` that nothing else
    // accesses during this call (caller contract).
    unsafe { (handle as *mut PdfExportJob).as_mut() }
        .ok_or_else(|| BridgeError::Rnote("The PDF export is closed".to_owned()))
}

/// Takes back the box of an export handle.
///
/// # Safety
///
/// As for [`job_from_handle`]; the handle must not be used again.
unsafe fn take_job(handle: jlong) -> Option<Box<PdfExportJob>> {
    let job = handle as *mut PdfExportJob;
    // SAFETY: A non-zero handle owns its box, given up here (caller contract).
    (!job.is_null()).then(|| unsafe { Box::from_raw(job) })
}

/// `static native long pdfExportBegin(long document, String title, boolean onWhitePaper,
/// int firstSheet, int lastSheet)`: starts the export of the document, on white paper or as
/// it is, of the sheets from `firstSheet` to `lastSheet` (0-based; -1 for every sheet);
/// called on the document's thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfExportBegin<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    document: jlong,
    title: JString<'caller>,
    on_white_paper: jni::sys::jboolean,
    first_sheet: jint,
    last_sheet: jint,
) -> jlong {
    unowned_env
        .with_env(|env| -> Result<jlong, BridgeError> {
            // SAFETY: `document` comes from `NativeDocument`, which upholds the invariants.
            let document = unsafe { document_from_handle(document) }?;
            let title = title.try_to_string(env)?;
            let sheets = usize::try_from(first_sheet).ok().zip(usize::try_from(last_sheet).ok());
            let options = PdfExportOptions { on_white_paper, sheets };
            let job = Box::new(document.pdf_export_job(&title, &options));
            Ok(Box::into_raw(job) as jlong)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int pdfExportTotal(long job)`: how many sheets the PDF has.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfExportTotal<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    job: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `job` comes from `PdfExport`, which upholds the module invariants.
            let job = unsafe { job_from_handle(job) }?;
            Ok(jint::try_from(job.total()).unwrap_or(jint::MAX))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int pdfExportDrawNext(long job)`: draws the next sheet; returns how many
/// are drawn.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfExportDrawNext<
    'caller,
>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    job: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `job` comes from `PdfExport`, which upholds the module invariants.
            let job = unsafe { job_from_handle(job) }?;
            job.draw_next()?;
            Ok(jint::try_from(job.done()).unwrap_or(jint::MAX))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native byte[] pdfExportFinish(long job)`: the PDF; the handle is closed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfExportFinish<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    job: jlong,
) -> JByteArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JByteArray<'caller>, BridgeError> {
            // SAFETY: `job` comes from `PdfExport`, which never uses it again.
            let job = unsafe { take_job(job) }
                .ok_or_else(|| BridgeError::Rnote("The PDF export is closed".to_owned()))?;
            let bytes = job.finish()?;
            Ok(env.byte_array_from_slice(&bytes)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void pdfExportCancel(long job)`: drops the export; the handle is closed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfExportCancel<'caller>(
    _env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    job: jlong,
) {
    // SAFETY: `job` comes from `PdfExport`, which never uses it again.
    drop(unsafe { take_job(job) });
}
