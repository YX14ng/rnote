//! Access to the pixels of an `android.graphics.Bitmap` through the NDK's `jnigraphics`.

use std::ffi::c_void;
use std::ptr;
use std::slice;

use anyhow::{Context, bail};
use jni::Env;
use jni::objects::JObject;
use jni::sys::{JNIEnv, jobject};

use crate::PixelLayout;

/// `ANDROID_BITMAP_RESULT_SUCCESS` from `android/bitmap.h`.
const ANDROID_BITMAP_RESULT_SUCCESS: i32 = 0;
/// `ANDROID_BITMAP_FORMAT_RGBA_8888` from `android/bitmap.h`.
const ANDROID_BITMAP_FORMAT_RGBA_8888: i32 = 1;

/// `AndroidBitmapInfo` from `android/bitmap.h`.
#[repr(C)]
#[derive(Debug, Default)]
struct AndroidBitmapInfo {
    width: u32,
    height: u32,
    stride: u32,
    format: i32,
    flags: u32,
}

#[link(name = "jnigraphics")]
unsafe extern "C" {
    fn AndroidBitmap_getInfo(
        env: *mut JNIEnv,
        jbitmap: jobject,
        info: *mut AndroidBitmapInfo,
    ) -> i32;
    fn AndroidBitmap_lockPixels(
        env: *mut JNIEnv,
        jbitmap: jobject,
        addr_ptr: *mut *mut c_void,
    ) -> i32;
    fn AndroidBitmap_unlockPixels(env: *mut JNIEnv, jbitmap: jobject) -> i32;
}

/// The locked pixels of an `RGBA_8888` bitmap. The bitmap is unlocked on drop.
pub struct LockedBitmap<'a> {
    env: *mut JNIEnv,
    bitmap: jobject,
    pixels: &'a mut [u8],
    layout: PixelLayout,
}

impl<'a> LockedBitmap<'a> {
    /// Locks the pixels of `bitmap`, which must be an `ARGB_8888` (`RGBA_8888` in the NDK)
    /// bitmap.
    ///
    /// Borrowing `env` mutably keeps any other JNI call from happening while the pixels
    /// are locked.
    ///
    /// # Safety
    ///
    /// No other code may read or write the bitmap's pixels while the returned value is
    /// alive, since they are exposed as a `&mut [u8]`.
    pub unsafe fn lock<'local>(
        env: &'a mut Env<'local>,
        bitmap: &'a JObject<'local>,
    ) -> anyhow::Result<Self> {
        if bitmap.is_null() {
            bail!("Bitmap is null");
        }
        let raw_env = env.get_raw();
        let raw_bitmap = bitmap.as_raw();

        let mut info = AndroidBitmapInfo::default();
        // SAFETY: `raw_env` is the JNI env of the current thread, `raw_bitmap` is a live local
        // reference to a non-null bitmap (both borrowed for 'a), and `info` is a valid
        // `AndroidBitmapInfo` to write into.
        let result = unsafe { AndroidBitmap_getInfo(raw_env, raw_bitmap, &mut info) };
        if result != ANDROID_BITMAP_RESULT_SUCCESS {
            bail!("AndroidBitmap_getInfo failed with {result}");
        }
        if info.format != ANDROID_BITMAP_FORMAT_RGBA_8888 {
            bail!("Unsupported bitmap format {}, expected RGBA_8888", info.format);
        }
        let layout = PixelLayout {
            width: info.width,
            height: info.height,
            stride: usize::try_from(info.stride).context("Bitmap stride too large")?,
        };
        let len = layout
            .stride
            .checked_mul(usize::try_from(info.height).context("Bitmap height too large")?)
            .context("Bitmap size overflows")?;

        let mut pixels_ptr: *mut c_void = ptr::null_mut();
        // SAFETY: Same env and bitmap as above; `pixels_ptr` is a valid out pointer.
        let result = unsafe { AndroidBitmap_lockPixels(raw_env, raw_bitmap, &mut pixels_ptr) };
        if result != ANDROID_BITMAP_RESULT_SUCCESS {
            bail!("AndroidBitmap_lockPixels failed with {result}");
        }
        if pixels_ptr.is_null() {
            // SAFETY: The pixels were successfully locked above, so they must be unlocked.
            unsafe { AndroidBitmap_unlockPixels(raw_env, raw_bitmap) };
            bail!("AndroidBitmap_lockPixels returned a null pointer");
        }

        // SAFETY: A successful lock guarantees that `pixels_ptr` points to `stride * height`
        // bytes that stay valid until the bitmap is unlocked, which only happens when this
        // value is dropped. The caller guarantees that nothing else accesses those bytes in
        // the meantime, so a unique slice is sound.
        let pixels = unsafe { slice::from_raw_parts_mut(pixels_ptr.cast::<u8>(), len) };

        Ok(Self { env: raw_env, bitmap: raw_bitmap, pixels, layout })
    }

    /// The pixels and their layout.
    pub fn pixels_mut(&mut self) -> (&mut [u8], PixelLayout) {
        (self.pixels, self.layout)
    }
}

impl Drop for LockedBitmap<'_> {
    fn drop(&mut self) {
        // SAFETY: `env` and `bitmap` were valid when locking and stay valid for 'a, which
        // outlives `self`. The bitmap is locked, since `lock` only returns on success.
        // Unlocking can only fail for invalid arguments, which this type rules out.
        unsafe { AndroidBitmap_unlockPixels(self.env, self.bitmap) };
    }
}
