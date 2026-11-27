//! Entry points for the pages of a document of sheets, and for inserting a PDF into one.
//!
//! A PDF pages handle is a pointer to a boxed [`PdfPages`], stored in a Java `long`. The
//! Kotlin `PageInserter` owns it: every handle comes from `pdfPagesBegin` and is passed to
//! either `insertPdfPages` or `pdfPagesClose` exactly once, never used afterwards, and never
//! from two threads at once.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JBooleanArray, JByteArray, JClass, JDoubleArray, JIntArray, JString};
use jni::sys::{jboolean, jint, jlong};

use super::common::{BridgeError, document_from_handle, edit, edit_flags};
use super::creation::optional_string;
use crate::pdf_import::PdfPages;

/// What `pageCount` returns for a document without sheets.
const NO_PAGES: jint = -1;

/// Resolves a PDF pages handle.
///
/// # Safety
///
/// `handle` must be 0 or a handle from `pdfPagesBegin` that wasn't closed or inserted, used
/// by no other call at the same time (see the module docs).
unsafe fn pages_from_handle<'a>(handle: jlong) -> Result<&'a mut PdfPages, BridgeError> {
    // SAFETY: Non-zero handles point to a live, boxed `PdfPages` that nothing else accesses
    // during this call (caller contract).
    unsafe { (handle as *mut PdfPages).as_mut() }
        .ok_or_else(|| BridgeError::Rnote("The PDF pages are closed".to_owned()))
}

fn page_index(value: jint) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

/// `static native int pageCount(long handle)`: the number of sheets, or -1 for a document
/// without sheets.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pageCount<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> jint {
    unowned_env
        .with_env(|_env| -> Result<jint, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(document
                .page_count()
                .map_or(NO_PAGES, |count| jint::try_from(count).unwrap_or(jint::MAX)))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native double[] pageSheet(long handle)`: `[width, height]` of a sheet, in document
/// coordinates.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pageSheet<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> JDoubleArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JDoubleArray<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let format = document.sheet_format();
            let array = env.new_double_array(2)?;
            array.set_region(env, 0, &[format.width(), format.height()])?;
            Ok(array)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native boolean[] pagesWithContent(long handle)`: whether each sheet has something
/// on it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pagesWithContent<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> JBooleanArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JBooleanArray<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let values: Vec<jboolean> = document.pages_with_content();
            let array = env.new_boolean_array(values.len())?;
            array.set_region(env, 0, &values)?;
            Ok(array)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int removePages(long handle, int[] pages)`: removes the sheets (0-based).
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_removePages<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    pages: JIntArray<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, BridgeError> {
            let mut values = vec![0; pages.len(env)?];
            pages.get_region(env, 0, &mut values)?;
            let pages: Vec<u32> = values.into_iter().map(page_index).collect();
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            Ok(edit_flags(document.remove_pages(&pages)))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int insertBlankPages(long handle, int at, int count)`: inserts `count`
/// blank sheets before sheet `at`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_insertBlankPages<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    at: jint,
    count: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        Ok(document.insert_blank_pages(page_index(at), page_index(count)))
    })
}

/// `static native int movePage(long handle, int from, int to)`: moves sheet `from` to be
/// sheet `to`, the ones between making room.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_movePage<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    from: jint,
    to: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        Ok(document.move_page(page_index(from), page_index(to)))
    })
}

/// `static native int duplicatePage(long handle, int page)`: puts a copy of sheet `page`
/// right after it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_duplicatePage<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    page: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| Ok(document.duplicate_page(page_index(page))))
}

/// `static native long pdfPagesBegin(long handle, byte[] pdf, String password, int
/// firstPage, int endPage, boolean vector)`: starts reading pages `firstPage` (0-based) up
/// to `endPage` (excluded) of `pdf` as wide as the document's sheets; throws if the PDF or
/// the pages can't be read.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfPagesBegin<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    pdf: JByteArray<'caller>,
    password: JString<'caller>,
    first_page: jint,
    end_page: jint,
    vector: jboolean,
) -> jlong {
    unowned_env
        .with_env(|env| -> Result<jlong, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let format = document.sheet_format();
            let bytes = env.convert_byte_array(&pdf)?;
            let password = optional_string(env, &password)?;
            let page = |page: jint| usize::try_from(page).unwrap_or(0);
            let pages =
                PdfPages::new(bytes, password, page(first_page)..page(end_page), vector, format)?;
            Ok(Box::into_raw(Box::new(pages)) as jlong)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native boolean pdfPagesNext(long pages)`: reads the next page; false once every
/// page is read.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfPagesNext<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    pages: jlong,
) -> jboolean {
    unowned_env
        .with_env(|_env| -> Result<jboolean, BridgeError> {
            // SAFETY: `pages` comes from `PageInserter`, which upholds the module contract.
            let pages = unsafe { pages_from_handle(pages) }?;
            Ok(pages.import_next()?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int insertPdfPages(long handle, long pages, int at)`: inserts the pages
/// read so far before sheet `at`, and frees `pages`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_insertPdfPages<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    pages: jlong,
    at: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| {
        if pages == 0 {
            return Err(BridgeError::Rnote("The PDF pages are closed".to_owned()));
        }
        // SAFETY: Non-zero handles come from `Box::into_raw` in `pdfPagesBegin`, and
        // `PageInserter` hands each one to this call or `pdfPagesClose` exactly once.
        let pages = unsafe { Box::from_raw(pages as *mut PdfPages) };
        Ok(document.insert_page_strokes(page_index(at), pages.into_pages()))
    })
}

/// `static native void pdfPagesClose(long pages)`: frees PDF pages that won't be inserted.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_pdfPagesClose<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    pages: jlong,
) {
    unowned_env
        .with_env(|_env| -> Result<(), BridgeError> {
            if pages != 0 {
                // SAFETY: Non-zero handles come from `Box::into_raw` in `pdfPagesBegin`, and
                // `PageInserter` closes each one exactly once and never uses it afterwards.
                drop(unsafe { Box::from_raw(pages as *mut PdfPages) });
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
