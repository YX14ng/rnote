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

/** What the editing UI shows: unsaved changes, undo/redo availability. */
data class EditState(
    val modified: Boolean,
    val canUndo: Boolean,
    val canRedo: Boolean,
    val boundsChanged: Boolean,
) {
    companion object {
        val CLEAN = EditState(modified = false, canUndo = false, canRedo = false, boundsChanged = false)

        // Mirrors the FLAG_* constants in jni_bridge.rs.
        private const val FLAG_MODIFIED = 1
        private const val FLAG_CAN_UNDO = 1 shl 1
        private const val FLAG_CAN_REDO = 1 shl 2
        private const val FLAG_BOUNDS_CHANGED = 1 shl 3

        fun fromFlags(flags: Int) = EditState(
            modified = flags and FLAG_MODIFIED != 0,
            canUndo = flags and FLAG_CAN_UNDO != 0,
            canRedo = flags and FLAG_CAN_REDO != 0,
            boundsChanged = flags and FLAG_BOUNDS_CHANGED != 0,
        )
    }
}

/** What a pen interaction does. Values match `jni_bridge.rs`. */
enum class Tool(val nativeValue: Int) {
    BRUSH(0),
    ERASER(1),
}

/** Where a batch of pen samples sits in an interaction. Values match `jni_bridge.rs`. */
enum class PenPhase(val nativeValue: Int) {
    DOWN(0),
    UP(1),
    CANCEL(2),
}

/**
 * A document loaded by the native engine.
 *
 * Not thread-safe: each instance must be created, used and closed on a single thread,
 * which the native side relies on. [DocumentRenderer] takes care of that.
 */
internal class NativeDocument private constructor(private var handle: Long) : AutoCloseable {
    val bounds: DocumentBounds
        get() {
            val raw = NativeBridge.bounds(checkOpen())
            return DocumentBounds(raw[0], raw[1], raw[2], raw[3])
        }

    val backgroundColor: Int
        get() = NativeBridge.backgroundColor(checkOpen())

    val editState: EditState
        get() = EditState.fromFlags(NativeBridge.editState(checkOpen()))

    /**
     * Renders [viewport] into [target].
     *
     * [target] must be an `ARGB_8888` bitmap that nothing else reads or writes until this
     * returns, because the native side writes its pixels directly.
     */
    fun render(target: Bitmap, viewport: Viewport) {
        NativeBridge.render(checkOpen(), target, viewport.x, viewport.y, viewport.zoom)
    }

    /** Feeds `x, y, pressure` triples in document coordinates to the engine. */
    fun handlePen(
        phase: PenPhase,
        tool: Tool,
        samples: DoubleArray,
        viewport: Viewport,
        viewWidth: Int,
        viewHeight: Int,
    ): EditState = EditState.fromFlags(
        NativeBridge.handlePen(
            checkOpen(),
            phase.nativeValue,
            tool.nativeValue,
            samples,
            viewport.x,
            viewport.y,
            viewport.zoom,
            viewWidth,
            viewHeight,
        ),
    )

    fun undo(): EditState = EditState.fromFlags(NativeBridge.undo(checkOpen()))

    fun redo(): EditState = EditState.fromFlags(NativeBridge.redo(checkOpen()))

    fun setBrush(brush: Brush) = NativeBridge.setBrush(checkOpen(), brush.color, brush.width)

    /** Serializes the document; call [markSaved] once the bytes are written. */
    fun save(fileName: String): ByteArray = NativeBridge.save(checkOpen(), fileName)

    fun markSaved() = NativeBridge.markSaved(checkOpen())

    override fun close() {
        if (handle != 0L) {
            NativeBridge.close(handle)
            handle = 0L
        }
    }

    private fun checkOpen(): Long {
        check(handle != 0L) { "Document is closed" }
        return handle
    }

    companion object {
        /** Loads a document from the contents of an `.rnote` file. */
        fun open(rnoteBytes: ByteArray): NativeDocument = NativeDocument(NativeBridge.open(rnoteBytes))

        /** Creates an empty document. */
        fun createEmpty(): NativeDocument = NativeDocument(NativeBridge.newDocument())
    }
}
