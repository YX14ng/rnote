package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap

/** Axis-aligned bounds in document coordinates. */
data class DocumentBounds(val left: Double, val top: Double, val right: Double, val bottom: Double) {
    val width: Double
        get() = right - left

    val height: Double
        get() = bottom - top
}

/** A point in document coordinates. */
data class DocumentPoint(val x: Double, val y: Double)

/** The region of the document shown on screen: its top-left corner and the zoom. */
data class Viewport(val x: Double, val y: Double, val zoom: Double)

/** What the editing UI shows: unsaved changes, undo/redo availability. */
data class EditState(
    val modified: Boolean,
    val canUndo: Boolean,
    val canRedo: Boolean,
    val boundsChanged: Boolean,
    val hasSelection: Boolean,
    val textInput: Boolean,
    /** The brush stroke being drawn just became a shape; the rest of the touch is ignored. */
    val shapeSnapped: Boolean = false,
) {
    companion object {
        val CLEAN = EditState(
            modified = false,
            canUndo = false,
            canRedo = false,
            boundsChanged = false,
            hasSelection = false,
            textInput = false,
        )

        // Mirrors the FLAG_* constants in jni_bridge/common.rs.
        private const val FLAG_MODIFIED = 1
        private const val FLAG_CAN_UNDO = 1 shl 1
        private const val FLAG_CAN_REDO = 1 shl 2
        private const val FLAG_BOUNDS_CHANGED = 1 shl 3
        private const val FLAG_HAS_SELECTION = 1 shl 4
        private const val FLAG_TEXT_INPUT = 1 shl 5
        private const val FLAG_SHAPE_SNAPPED = 1 shl 6

        fun fromFlags(flags: Int) = EditState(
            modified = flags and FLAG_MODIFIED != 0,
            canUndo = flags and FLAG_CAN_UNDO != 0,
            canRedo = flags and FLAG_CAN_REDO != 0,
            boundsChanged = flags and FLAG_BOUNDS_CHANGED != 0,
            hasSelection = flags and FLAG_HAS_SELECTION != 0,
            textInput = flags and FLAG_TEXT_INPUT != 0,
            shapeSnapped = flags and FLAG_SHAPE_SNAPPED != 0,
        )
    }
}

/** What a pen interaction does. Values match `jni_bridge.rs`. */
enum class Tool(val nativeValue: Int) {
    BRUSH(0),
    ERASER(1),
    SHAPER(2),
    SELECTOR(3),
    TYPEWRITER(4),
}

/**
 * Shapes the shaper offers. Values are `ShapeBuilderType` discriminants in rnote-compose.
 *
 * Curves are built by dragging from the start to the control point(s), then tapping the end;
 * they finish on their own. [needsFinish] shapes (polylines and polygons) take one tap per
 * point and end with a tap on the last point or with [EditingKey.ENTER].
 */
enum class Shape(val nativeValue: Int, val label: Int, val needsFinish: Boolean = false) {
    LINE(0, R.string.shape_line),
    ARROW(1, R.string.shape_arrow),
    RECTANGLE(2, R.string.shape_rectangle),
    GRID(3, R.string.shape_grid),
    COORDINATE_SYSTEM(4, R.string.shape_coordinate_system),
    COORDINATE_SYSTEM_3D(5, R.string.shape_coordinate_system_3d),
    QUADRANT_COORDINATE_SYSTEM(6, R.string.shape_quadrant_coordinate_system),
    ELLIPSE(7, R.string.shape_ellipse),
    FOCI_ELLIPSE(8, R.string.shape_foci_ellipse),
    QUADRATIC_CURVE(9, R.string.shape_quadratic_curve),
    CUBIC_CURVE(10, R.string.shape_cubic_curve),
    POLYLINE(11, R.string.shape_polyline, needsFinish = true),
    POLYGON(12, R.string.shape_polygon, needsFinish = true),
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

/** Emphasis that can be toggled on selected text. Values match `jni_bridge/editing.rs`. */
enum class TextEmphasis(val nativeValue: Int, val label: Int) {
    BOLD(0, R.string.emphasis_bold),
    ITALIC(1, R.string.emphasis_italic),
    UNDERLINE(2, R.string.emphasis_underline),
    STRIKETHROUGH(3, R.string.emphasis_strikethrough),
}

/** Where a batch of pen samples sits in an interaction. Values match `jni_bridge.rs`. */
enum class PenPhase(val nativeValue: Int) {
    DOWN(0),
    UP(1),
    CANCEL(2),

    /** The pen hovers without touching. */
    PROXIMITY(3),
}

/** Keys that edit text or finish shapes. Values match `key_from_int` in `jni_bridge.rs`. */
enum class EditingKey(val nativeValue: Int) {
    BACKSPACE(0),
    DELETE(1),
    ENTER(2),
    ESCAPE(3),
    LEFT(4),
    RIGHT(5),
    UP(6),
    DOWN(7),
    HOME(8),
    END(9),
    TAB(10),
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

    /** The top-left corner of what the view showed when the document was saved. */
    val savedViewOrigin: DocumentPoint
        get() {
            val raw = NativeBridge.savedViewOrigin(checkOpen())
            return DocumentPoint(raw[0], raw[1])
        }

    val backgroundColor: Int
        get() = NativeBridge.backgroundColor(checkOpen())

    val editState: EditState
        get() = EditState.fromFlags(NativeBridge.editState(checkOpen()))

    val settings: DocumentSettings
        get() = DocumentSettings.fromArray(NativeBridge.documentSettings(checkOpen()))

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

    /** Turns the brush stroke being drawn into the shape it looks like, if any. */
    fun snapToShape(viewport: Viewport, viewWidth: Int, viewHeight: Int): EditState = EditState.fromFlags(
        NativeBridge.snapToShape(checkOpen(), viewport.x, viewport.y, viewport.zoom, viewWidth, viewHeight),
    )

    fun setTool(tool: Tool): EditState = EditState.fromFlags(NativeBridge.setTool(checkOpen(), tool.nativeValue))

    fun setShape(shape: Shape): EditState = EditState.fromFlags(NativeBridge.setShape(checkOpen(), shape.nativeValue))

    fun setBrushStyle(style: BrushStyle): EditState =
        EditState.fromFlags(NativeBridge.setBrushStyle(checkOpen(), style.nativeValue))

    fun setEraserStyle(style: EraserStyle): EditState =
        EditState.fromFlags(NativeBridge.setEraserStyle(checkOpen(), style.nativeValue))

    fun selection(action: SelectionAction): EditState =
        EditState.fromFlags(NativeBridge.selection(checkOpen(), action.nativeValue))

    fun handleKey(key: EditingKey, shift: Boolean, ctrl: Boolean): EditState =
        EditState.fromFlags(NativeBridge.handleKey(checkOpen(), key.nativeValue, shift, ctrl))

    fun handleCharKey(codePoint: Int, shift: Boolean, ctrl: Boolean): EditState =
        EditState.fromFlags(NativeBridge.handleCharKey(checkOpen(), codePoint, shift, ctrl))

    fun toggleTextEmphasis(emphasis: TextEmphasis): EditState =
        EditState.fromFlags(NativeBridge.textEmphasis(checkOpen(), emphasis.nativeValue))

    fun colorSelectedText(color: Int): EditState =
        EditState.fromFlags(NativeBridge.colorSelectedText(checkOpen(), color))

    /** What is selected, or null if nothing is. */
    fun copy(): ClipContent? = ClipContent.decode(NativeBridge.copy(checkOpen(), cut = false))

    /** Like [copy], then removes what was copied. */
    fun cut(): Pair<ClipContent?, EditState> {
        val content = ClipContent.decode(NativeBridge.copy(checkOpen(), cut = true))
        return content to editState
    }

    fun pasteStrokeContent(json: String, x: Double, y: Double): EditState =
        EditState.fromFlags(NativeBridge.pasteStrokeContent(checkOpen(), json, x, y))

    fun pasteText(text: String, x: Double, y: Double): EditState =
        EditState.fromFlags(NativeBridge.pasteText(checkOpen(), text, x, y))

    fun pasteImage(bytes: ByteArray, x: Double, y: Double): EditState =
        EditState.fromFlags(NativeBridge.pasteImage(checkOpen(), bytes, x, y))

    fun handleText(text: String): EditState = EditState.fromFlags(NativeBridge.handleText(checkOpen(), text))

    fun setTextFormat(format: TextFormat): EditState = EditState.fromFlags(
        NativeBridge.setTextFormat(
            checkOpen(),
            format.fontFamily.family,
            format.fontSize,
            format.bold,
            format.italic,
            format.alignment.nativeValue,
        ),
    )

    fun setShapeStyle(style: ShapeStyle): EditState = EditState.fromFlags(
        NativeBridge.setShapeStyle(checkOpen(), style.rough, style.fill != null, style.fill ?: 0),
    )

    fun setPageSize(widthMm: Double, heightMm: Double): EditState =
        EditState.fromFlags(NativeBridge.setPageSize(checkOpen(), widthMm, heightMm))

    fun setLayout(layout: DocumentLayout): EditState =
        EditState.fromFlags(NativeBridge.setLayout(checkOpen(), layout.nativeValue))

    fun setBackground(color: Int, pattern: Pattern, patternColor: Int, patternSize: Double): EditState =
        EditState.fromFlags(
            NativeBridge.setBackground(checkOpen(), color, pattern.nativeValue, patternColor, patternSize),
        )

    fun setPageBorders(show: Boolean): EditState = EditState.fromFlags(NativeBridge.setPageBorders(checkOpen(), show))

    /** A page more at the end, in a document of pages; not undoable. */
    fun addPage(): EditState = EditState.fromFlags(NativeBridge.addPage(checkOpen()))

    fun setShapeRecognition(enabled: Boolean, minSizePx: Double): EditState =
        EditState.fromFlags(NativeBridge.setShapeRecognition(checkOpen(), enabled, minSizePx))

    /** Where the selected strokes are, if any. */
    fun selectionBounds(): DocumentBounds? {
        val raw = NativeBridge.selectionBounds(checkOpen())
        return if (raw.size == 4) DocumentBounds(raw[0], raw[1], raw[2], raw[3]) else null
    }

    fun recolorSelection(color: Int): EditState =
        EditState.fromFlags(NativeBridge.recolorSelection(checkOpen(), color))

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
