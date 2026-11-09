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

    /** Creates an empty document and returns its handle. */
    @JvmStatic external fun newDocument(): Long

    /** Document bounds as `[minX, minY, maxX, maxY]`. */
    @JvmStatic external fun bounds(handle: Long): DoubleArray

    /** The document background as an `android.graphics.Color` int. */
    @JvmStatic external fun backgroundColor(handle: Long): Int

    /** The editing state, packed as in [EditState.fromFlags]. */
    @JvmStatic external fun editState(handle: Long): Int

    /** Renders the region at ([x], [y]) in document coordinates into [target]. */
    @JvmStatic external fun render(handle: Long, target: Bitmap, x: Double, y: Double, zoom: Double)

    /**
     * Feeds stylus [samples] (`x, y, pressure` triples in document coordinates) to the
     * engine. Returns the editing state packed as in [EditState.fromFlags].
     */
    @JvmStatic external fun handlePen(
        handle: Long,
        phase: Int,
        tool: Int,
        samples: DoubleArray,
        x: Double,
        y: Double,
        zoom: Double,
        viewWidth: Int,
        viewHeight: Int,
    ): Int

    /** Switches the pen-side tool, see [Tool]. Returns the packed editing state. */
    @JvmStatic external fun setTool(handle: Long, tool: Int): Int

    /** Sets the shape the shaper draws, see [Shape]. Returns the packed editing state. */
    @JvmStatic external fun setShape(handle: Long, shape: Int): Int

    /** Sets the look of brush strokes, see [BrushStyle]. Returns the packed editing state. */
    @JvmStatic external fun setBrushStyle(handle: Long, style: Int): Int

    /** Sets what the eraser removes, see [EraserStyle]. Returns the packed editing state. */
    @JvmStatic external fun setEraserStyle(handle: Long, style: Int): Int

    /** Runs a [SelectionAction]. Returns the packed editing state. */
    @JvmStatic external fun selection(handle: Long, action: Int): Int

    /** Marks the document as having unsaved changes. Returns the packed editing state. */
    @JvmStatic external fun markModified(handle: Long): Int

    @JvmStatic external fun undo(handle: Long): Int

    @JvmStatic external fun redo(handle: Long): Int

    /** Sets the color (an `android.graphics.Color` int) and width of new brush strokes. */
    @JvmStatic external fun setBrush(handle: Long, argb: Int, width: Double)

    /** Serializes the document as an `.rnote` file. */
    @JvmStatic external fun save(handle: Long, fileName: String): ByteArray

    /** Marks the document as saved, once the bytes from [save] are written. */
    @JvmStatic external fun markSaved(handle: Long)

    /** Frees the document. The handle must not be used afterwards. */
    @JvmStatic external fun close(handle: Long)
}
