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
    val hasSelection: Boolean,
) {
    companion object {
        val CLEAN = EditState(
            modified = false,
            canUndo = false,
            canRedo = false,
            boundsChanged = false,
            hasSelection = false,
        )

        // Mirrors the FLAG_* constants in jni_bridge.rs.
        private const val FLAG_MODIFIED = 1
        private const val FLAG_CAN_UNDO = 1 shl 1
        private const val FLAG_CAN_REDO = 1 shl 2
        private const val FLAG_BOUNDS_CHANGED = 1 shl 3
        private const val FLAG_HAS_SELECTION = 1 shl 4

        fun fromFlags(flags: Int) = EditState(
            modified = flags and FLAG_MODIFIED != 0,
            canUndo = flags and FLAG_CAN_UNDO != 0,
            canRedo = flags and FLAG_CAN_REDO != 0,
            boundsChanged = flags and FLAG_BOUNDS_CHANGED != 0,
            hasSelection = flags and FLAG_HAS_SELECTION != 0,
        )
    }
}

/** What a pen interaction does. Values match `jni_bridge.rs`. */
enum class Tool(val nativeValue: Int) {
    BRUSH(0),
    ERASER(1),
    SHAPER(2),
    SELECTOR(3),
}

/** Shapes the shaper offers. Values are `ShapeBuilderType` discriminants in rnote-compose. */
enum class Shape(val nativeValue: Int, val label: Int) {
    LINE(0, R.string.shape_line),
    ARROW(1, R.string.shape_arrow),
    RECTANGLE(2, R.string.shape_rectangle),
    GRID(3, R.string.shape_grid),
    COORDINATE_SYSTEM(4, R.string.shape_coordinate_system),
    ELLIPSE(7, R.string.shape_ellipse),
}

/** Brush stroke looks. Values are `BrushStyle` discriminants in rnote-engine. */
enum class BrushStyle(val nativeValue: Int, val label: Int) {
    MARKER(0, R.string.brush_style_marker),
    SOLID(1, R.string.brush_style_solid),
    TEXTURED(2, R.string.brush_style_textured),
}

/** What the eraser removes. Values are `EraserStyle` discriminants in rnote-engine. */
enum class EraserStyle(val nativeValue: Int, val label: Int) {
    WHOLE_STROKES(0, R.string.eraser_whole_strokes),
    SPLIT_STROKES(1, R.string.eraser_split_strokes),
}

/** Operations on the selection. Values match `jni_bridge.rs`. */
enum class SelectionAction(val nativeValue: Int) {
    SELECT_ALL(0),
    DESELECT_ALL(1),
    DELETE(2),
    DUPLICATE(3),
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

    fun setTool(tool: Tool): EditState = EditState.fromFlags(NativeBridge.setTool(checkOpen(), tool.nativeValue))

    fun setShape(shape: Shape): EditState = EditState.fromFlags(NativeBridge.setShape(checkOpen(), shape.nativeValue))

    fun setBrushStyle(style: BrushStyle): EditState =
        EditState.fromFlags(NativeBridge.setBrushStyle(checkOpen(), style.nativeValue))

    fun setEraserStyle(style: EraserStyle): EditState =
        EditState.fromFlags(NativeBridge.setEraserStyle(checkOpen(), style.nativeValue))

    fun selection(action: SelectionAction): EditState =
        EditState.fromFlags(NativeBridge.selection(checkOpen(), action.nativeValue))

    fun markModified(): EditState = EditState.fromFlags(NativeBridge.markModified(checkOpen()))

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
