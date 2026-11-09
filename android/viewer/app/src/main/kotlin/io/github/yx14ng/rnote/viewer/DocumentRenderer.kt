package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/** A rendered region of the document, and the last edit it includes. */
class Frame(val bitmap: Bitmap, val viewport: Viewport, val editSequence: Int)

/** A freshly opened or created document. */
class DocumentInfo(val bounds: DocumentBounds, val backgroundColor: Int, val editState: EditState)

/** The outcome of an edit; [bounds] is only set when they changed. */
class EditResult(val state: EditState, val bounds: DocumentBounds?)

/**
 * Owns the [NativeDocument] on a dedicated thread.
 *
 * All native calls happen on that single thread, in submission order, as [NativeDocument]
 * requires. So a render submitted after an edit always includes it. Results are delivered
 * on the main thread.
 */
internal class DocumentRenderer : AutoCloseable {
    private val executor: ExecutorService =
        Executors.newSingleThreadExecutor { runnable -> Thread(runnable, "rnote-document") }
    private val mainHandler = Handler(Looper.getMainLooper())

    // Only accessed on the executor thread.
    private var document: NativeDocument? = null

    /** Replaces the current document with the one whose bytes [load] reads. */
    fun open(load: () -> ByteArray, brush: Brush?, onResult: (Result<DocumentInfo>) -> Unit) =
        replaceDocument({ NativeDocument.open(load()) }, brush, onResult)

    /** Replaces the current document with an empty one. */
    fun createEmpty(brush: Brush?, onResult: (Result<DocumentInfo>) -> Unit) =
        replaceDocument(NativeDocument::createEmpty, brush, onResult)

    /** Runs [operation] on the document and reports the new editing state. */
    fun edit(operation: (NativeDocument) -> EditState, onResult: (Result<EditResult>) -> Unit) =
        submit(onResult) { current ->
            val state = operation(current)
            EditResult(state, if (state.boundsChanged) current.bounds else null)
        }

    /** Applies [brush] to the strokes drawn from now on. */
    fun setBrush(brush: Brush) = submit<Unit>({}) { current -> current.setBrush(brush) }

    /**
     * Serializes the document, hands the bytes to [write] (still off the main thread) and
     * marks the document as saved once that succeeds.
     */
    fun save(fileName: String, write: (ByteArray) -> Unit, onResult: (Result<EditState>) -> Unit) =
        submit(onResult) { current ->
            write(current.save(fileName))
            current.markSaved()
            current.editState
        }

    /**
     * Serializes the document and hands the bytes to [write] without marking it as saved,
     * e.g. for a recovery copy.
     */
    fun saveCopy(fileName: String, write: (ByteArray) -> Unit, onResult: (Result<Unit>) -> Unit) =
        submit(onResult) { current -> write(current.save(fileName)) }

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
        editSequence: Int,
        onFrame: (Result<Frame>) -> Unit,
    ) = submit(onFrame) { current ->
        val bitmap =
            reuse?.takeIf { it.width == width && it.height == height && !it.isRecycled }
                ?: Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        current.render(bitmap, viewport)
        Frame(bitmap, viewport, editSequence)
    }

    override fun close() {
        executor.execute {
            document?.close()
            document = null
        }
        executor.shutdown()
    }

    private fun replaceDocument(
        create: () -> NativeDocument,
        brush: Brush?,
        onResult: (Result<DocumentInfo>) -> Unit,
    ) {
        executor.execute {
            val result = runCatching {
                val created = create()
                document?.close()
                document = created
                brush?.let(created::setBrush)
                DocumentInfo(created.bounds, created.backgroundColor, created.editState)
            }
            mainHandler.post { onResult(result) }
        }
    }

    private fun <T> submit(onResult: (Result<T>) -> Unit, task: (NativeDocument) -> T) {
        executor.execute {
            val result = runCatching { task(checkNotNull(document) { "No document is open" }) }
            mainHandler.post { onResult(result) }
        }
    }
}
