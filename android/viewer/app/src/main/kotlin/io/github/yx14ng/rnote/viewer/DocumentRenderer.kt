package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/** A rendered region of the document. */
class Frame(val bitmap: Bitmap, val viewport: Viewport)

/**
 * Loads and renders a [NativeDocument] on a dedicated thread.
 *
 * All native calls happen on that single thread, as [NativeDocument] requires. Results are
 * delivered on the main thread.
 */
internal class DocumentRenderer : AutoCloseable {
    private val executor: ExecutorService =
        Executors.newSingleThreadExecutor { runnable -> Thread(runnable, "rnote-renderer") }
    private val mainHandler = Handler(Looper.getMainLooper())

    // Only accessed on the executor thread.
    private var document: NativeDocument? = null

    /** Replaces the current document with the one whose bytes [load] reads. */
    fun open(load: () -> ByteArray, onResult: (Result<DocumentBounds>) -> Unit) {
        executor.execute {
            val result = runCatching {
                val opened = NativeDocument.open(load())
                document?.close()
                document = opened
                opened.bounds
            }
            mainHandler.post { onResult(result) }
        }
    }

    /**
     * Renders [viewport] at [width] x [height] pixels.
     *
     * [reuse] is drawn into when it has the right size; it must not be displayed anymore.
     */
    fun render(
        viewport: Viewport,
        width: Int,
        height: Int,
        reuse: Bitmap?,
        onFrame: (Result<Frame>) -> Unit,
    ) {
        executor.execute {
            val result = runCatching {
                val current = checkNotNull(document) { "No document is open" }
                val bitmap =
                    reuse?.takeIf { it.width == width && it.height == height && !it.isRecycled }
                        ?: Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
                current.render(bitmap, viewport)
                Frame(bitmap, viewport)
            }
            mainHandler.post { onFrame(result) }
        }
    }

    override fun close() {
        executor.execute {
            document?.close()
            document = null
        }
        executor.shutdown()
    }
}
