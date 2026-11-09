package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap

/** Axis-aligned bounds in document coordinates. */
data class DocumentBounds(val left: Double, val top: Double, val right: Double, val bottom: Double) {
    val width: Double
        get() = right - left

    val height: Double
        get() = bottom - top
}

/** The region of the document shown on screen: its top-left corner and the zoom. */
data class Viewport(val x: Double, val y: Double, val zoom: Double)

/**
 * A document loaded by the native engine.
 *
 * Not thread-safe: each instance must be created, used and closed on a single thread,
 * which the native side relies on. [DocumentRenderer] takes care of that.
 */
internal class NativeDocument private constructor(private var handle: Long) : AutoCloseable {
    val bounds: DocumentBounds

    init {
        try {
            val raw = NativeBridge.bounds(handle)
            bounds = DocumentBounds(raw[0], raw[1], raw[2], raw[3])
        } catch (e: RuntimeException) {
            close()
            throw e
        }
    }

    /**
     * Renders [viewport] into [target].
     *
     * [target] must be an `ARGB_8888` bitmap that nothing else reads or writes until this
     * returns, because the native side writes its pixels directly.
     */
    fun render(target: Bitmap, viewport: Viewport) {
        check(handle != 0L) { "Document is closed" }
        NativeBridge.render(handle, target, viewport.x, viewport.y, viewport.zoom)
    }

    override fun close() {
        if (handle != 0L) {
            NativeBridge.close(handle)
            handle = 0L
        }
    }

    companion object {
        /** Loads a document from the contents of an `.rnote` file. */
        fun open(rnoteBytes: ByteArray): NativeDocument = NativeDocument(NativeBridge.open(rnoteBytes))
    }
}
