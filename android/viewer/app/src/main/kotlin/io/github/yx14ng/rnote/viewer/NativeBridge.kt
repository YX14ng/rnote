package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap

/**
 * JNI entry points implemented in `crates/rnote-android/src/jni_bridge.rs`.
 *
 * Only [NativeDocument] calls these, since it upholds the invariants the native side
 * relies on.
 */
internal object NativeBridge {
    init {
        System.loadLibrary("rnote_android")
    }

    /** Loads a document and returns its handle. Throws on invalid files. */
    @JvmStatic external fun open(rnoteBytes: ByteArray): Long

    /** Document bounds as `[minX, minY, maxX, maxY]`. */
    @JvmStatic external fun bounds(handle: Long): DoubleArray

    /** Renders the region at ([x], [y]) in document coordinates into [target]. */
    @JvmStatic external fun render(handle: Long, target: Bitmap, x: Double, y: Double, zoom: Double)

    /** Frees the document. The handle must not be used afterwards. */
    @JvmStatic external fun close(handle: Long)
}
