package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap

/**
 * JNI entry points implemented in `crates/rnote-android/src/jni_bridge.rs`.
 *
 * Only [NativeDocument] calls the document functions, since it upholds the invariants the
 * native side relies on; [LibraryStore] calls the library ones.
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

    /** Page size, layout and background; see [DocumentSettings.fromArray]. */
    @JvmStatic external fun documentSettings(handle: Long): DoubleArray

    /** Sets the format of new text and of the text being edited. Returns the packed state. */
    @JvmStatic external fun setTextFormat(
        handle: Long,
        fontFamily: String,
        fontSize: Double,
        bold: Boolean,
        italic: Boolean,
        alignment: Int,
    ): Int

    /** Sets the style and fill of new shapes. Returns the packed editing state. */
    @JvmStatic external fun setShapeStyle(handle: Long, rough: Boolean, filled: Boolean, fillArgb: Int): Int

    @JvmStatic external fun setPageSize(handle: Long, widthMm: Double, heightMm: Double): Int

    @JvmStatic external fun setLayout(handle: Long, layout: Int): Int

    /** Sets the background color, pattern, pattern color and pattern spacing (document units). */
    @JvmStatic external fun setBackground(
        handle: Long,
        argb: Int,
        pattern: Int,
        patternArgb: Int,
        patternSize: Double,
    ): Int

    @JvmStatic external fun setPageBorders(handle: Long, show: Boolean): Int

    /** The selection's bounds as `[minX, minY, maxX, maxY]`, or empty if nothing is selected. */
    @JvmStatic external fun selectionBounds(handle: Long): DoubleArray

    /** Gives the selected strokes a color. Returns the packed editing state. */
    @JvmStatic external fun recolorSelection(handle: Long, argb: Int): Int

    /**
     * Turns the replacement of brush strokes that look like shapes on or off; strokes smaller
     * than [minSizePx] screen pixels are kept. Returns the packed editing state.
     */
    @JvmStatic external fun setShapeRecognition(handle: Long, enabled: Boolean, minSizePx: Double): Int

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

    /**
     * Replaces the brush stroke being drawn with the shape it looks like, if shape recognition
     * is on and it looks like one. Returns the packed editing state.
     */
    @JvmStatic external fun snapToShape(
        handle: Long,
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

    /** Feeds an [EditingKey] to the current pen. Returns the packed editing state. */
    @JvmStatic external fun handleKey(handle: Long, key: Int, shift: Boolean, ctrl: Boolean): Int

    /** Feeds a character key with modifiers, e.g. Ctrl+A in the text tool. */
    @JvmStatic external fun handleCharKey(handle: Long, codePoint: Int, shift: Boolean, ctrl: Boolean): Int

    /** Toggles a [TextEmphasis] on the selected text. Returns the packed editing state. */
    @JvmStatic external fun textEmphasis(handle: Long, emphasis: Int): Int

    /** Colors the selected text. Returns the packed editing state. */
    @JvmStatic external fun colorSelectedText(handle: Long, argb: Int): Int

    /**
     * What is selected, as `"<mime type>\n<data>"`, or null if nothing is. With [cut], it is
     * removed from the document.
     */
    @JvmStatic external fun copy(handle: Long, cut: Boolean): String?

    /** Pastes Rnote stroke content centered at ([x], [y]). Returns the packed editing state. */
    @JvmStatic external fun pasteStrokeContent(handle: Long, json: String, x: Double, y: Double): Int

    /** Pastes text in a text box starting at ([x], [y]). Returns the packed editing state. */
    @JvmStatic external fun pasteText(handle: Long, text: String, x: Double, y: Double): Int

    /** Pastes an encoded image centered at ([x], [y]). Returns the packed editing state. */
    @JvmStatic external fun pasteImage(handle: Long, bytes: ByteArray, x: Double, y: Double): Int

    /** Inserts committed text at the text tool's cursor. Returns the packed editing state. */
    @JvmStatic external fun handleText(handle: Long, text: String): Int

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

    /** Runs a JSON request on the library at [root]; returns the JSON answer, throws on errors. */
    @JvmStatic external fun libraryCall(root: String, request: String): String

    /** A PNG of the notebook's first page, at most [sizePx] wide and high, or null. */
    @JvmStatic external fun libraryThumbnail(rnoteBytes: ByteArray, sizePx: Int): ByteArray?

    /**
     * Saves the open notebook to [path] in the library at [root], or to a conflict copy beside
     * it if the file isn't the one with hash [expectedSha256] anymore; returns the JSON
     * `{"path", "sha256", "conflict"}`, throws on errors.
     */
    @JvmStatic external fun libraryWriteNotebook(
        root: String,
        path: String,
        expectedSha256: String?,
        device: String,
        bytes: ByteArray,
    ): String

    /** Syncs the library at [root] with Google Drive; returns the JSON outcome. Blocks on the network. */
    @JvmStatic external fun syncRun(root: String, statePath: String, device: String, token: String): String

    /** Revokes the access [token], signing out of Google Drive; throws on errors. Blocks on the network. */
    @JvmStatic external fun syncRevoke(token: String)
}
