//! Entry points that import a PDF as a new notebook, one page per call.
//!
//! An import handle is a pointer to a boxed [`PdfImport`], stored in a Java `long`. The
//! Kotlin `PdfImporter` owns it: every handle comes from `pdfImportBegin` and is passed to
//! `pdfImportClose` exactly once, never used afterwards, and never from two threads at once.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jboolean, jint, jlong};

use super::common::BridgeError;
use super::creation::{optional_string, setup_from_json};
use crate::pdf_import::{PdfImport, PdfOpenError, pdf_page_count};

/// What `pdfPageCount` returns for a PDF that needs a (different) password.
const NEEDS_PASSWORD: jint = -1;

impl From<PdfOpenError> for BridgeError {
    fn from(error: PdfOpenError) -> Self {
        match error {
            PdfOpenError::NeedsPassword => Self::Rnote("The PDF needs a password".to_owned()),
            PdfOpenError::Invalid(error) => error.into(),
        }
    }
}

/// Resolves an import handle.
///
/// # Safety
///
/// `handle` must be 0 or a handle from `pdfImportBegin` that wasn't closed, used by no
/// other call at the same time (see the module docs).
unsafe fn import_from_handle<'a>(handle: jlong) -> Result<&'a mut PdfImport, BridgeError> {
    // SAFETY: Non-zero handles point to a live, boxed `PdfImport` that nothing else
    // accesses during this call (caller contract).
    unsafe { (handle as *mut PdfImport).as_mut() }
        .ok_or_else(|| BridgeError::Rnote("The PDF import is closed".to_owned()))
}

/// `static native int pdfPageCount(byte[] pdf, String password)`: the number of pages, or
/// -1 if the PDF needs a password (none, or a wrong one, was given); throws if it can't be
/// read.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfPageCount<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    pdf: JByteArray<'caller>,
    password: JString<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            let bytes = env.convert_byte_array(&pdf)?;
            let password = optional_string(env, &password)?;
            match pdf_page_count(&bytes, password.as_deref()) {
                Ok(count) => Ok(jint::try_from(count).unwrap_or(jint::MAX)),
                Err(PdfOpenError::NeedsPassword) => Ok(NEEDS_PASSWORD),
                Err(error) => Err(error.into()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native long pdfImportBegin(byte[] pdf, String password, String setupJson, int
/// firstPage, int endPage, boolean vector)`: starts importing pages `firstPage` (0-based)
/// up to `endPage` (excluded); throws if the PDF or the pages can't be read.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfImportBegin<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    pdf: JByteArray<'caller>,
    password: JString<'caller>,
    setup_json: JString<'caller>,
    first_page: jint,
    end_page: jint,
    vector: jboolean,
) -> jlong {
    unowned_env
        .with_env(|env| -> Result<jlong, BridgeError> {
            let bytes = env.convert_byte_array(&pdf)?;
            let password = optional_string(env, &password)?;
            let setup = setup_from_json(&setup_json.try_to_string(env)?)?;
            let page = |page: jint| usize::try_from(page).unwrap_or(0);
            let pages = page(first_page)..page(end_page);
            let import = PdfImport::new(bytes, password, &setup, pages, vector)?;
            Ok(Box::into_raw(Box::new(import)) as jlong)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int pdfImportTotal(long handle)`: the number of pages to import.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfImportTotal<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `PdfImporter`, which upholds the module contract.
            let import = unsafe { import_from_handle(handle) }?;
            Ok(jint::try_from(import.total()).unwrap_or(jint::MAX))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native boolean pdfImportNext(long handle)`: imports the next page; false once
/// every page is imported.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfImportNext<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jboolean {
    unowned_env
        .with_env(|_env| -> Result<jboolean, BridgeError> {
            // SAFETY: `handle` comes from `PdfImporter`, which upholds the module contract.
            let import = unsafe { import_from_handle(handle) }?;
            Ok(import.import_next()?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native byte[] pdfImportFinish(long handle, String fileName)`: the `.rnote`
/// bytes of the notebook with the imported pages.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfImportFinish<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    file_name: JString<'caller>,
) -> JObject<'caller> {
    unowned_env
        .with_env(|env| -> Result<JObject<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `PdfImporter`, which upholds the module contract.
            let import = unsafe { import_from_handle(handle) }?;
            let bytes = import.finish(&file_name.try_to_string(env)?)?;
            Ok(env.byte_array_from_slice(&bytes)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native void pdfImportClose(long handle)`: frees the import, finished or not.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfImportClose<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) {
    unowned_env
        .with_env(|_env| -> Result<(), BridgeError> {
            if handle != 0 {
                // SAFETY: Non-zero handles come from `Box::into_raw` in `pdfImportBegin`, and
                // `PdfImporter` closes each one exactly once and never uses it afterwards.
                drop(unsafe { Box::from_raw(handle as *mut PdfImport) });
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
