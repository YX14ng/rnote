package io.github.yx14ng.rnote.viewer

import android.content.Context

/**
 * The editing choices (tool, brush, shapes, text, ...): kept in [EditorSettings] across
 * restarts, shown by the canvas and the tool sidebar, and applied to the open document.
 *
 * [onChanged] runs after every change, e.g. to refresh the menu.
 */
internal class EditorControls(
    private val context: Context,
    private val canvas: NoteCanvasView,
    private val sidebar: ToolSidebar,
    private val renderer: DocumentRenderer,
    private val onChanged: () -> Unit,
) {
    var settings: EditorSettings = EditorSettings.load(context)
        private set

    /** Whether a document is open, so brush changes reach it. */
    var documentOpen = false

    init {
        applyToView()
    }

    /** Saves [newSettings] and shows them. Callers apply them to the document as needed. */
    fun change(newSettings: EditorSettings) {
        settings = newSettings
        settings.save(context)
        applyToView()
        onChanged()
    }

    fun selectTool(tool: Tool) {
        change(settings.copy(tool = tool))
        canvas.applyEdit { it.setTool(tool) }
    }

    /** Applies [brush]; [colorChosen] tells whether its color was picked rather than automatic. */
    fun changeBrush(brush: Brush, colorChosen: Boolean) {
        change(settings.copy(brush = brush, colorChosen = colorChosen))
        if (documentOpen) {
            renderer.setBrush(brush)
        }
    }

    /** Draws with the palette color in [slot]; the eraser or lasso give way to the pen. */
    fun selectPaletteColor(slot: Int) {
        val color = settings.palette.getOrNull(slot) ?: return
        changeBrush(settings.brush.copy(color = color), colorChosen = true)
        if (settings.tool == Tool.ERASER || settings.tool == Tool.SELECTOR) {
            selectTool(Tool.BRUSH)
        }
    }

    /** Lets the user change the palette color in [slot], which then becomes the pen's. */
    fun editPaletteColor(slot: Int) {
        val current = settings.palette.getOrNull(slot) ?: return
        ColorPicker.show(context, R.string.title_palette_color, current) { color ->
            val palette = settings.palette.toMutableList().also { it[slot] = color }
            change(settings.copy(palette = palette))
            selectPaletteColor(slot)
        }
    }

    /** Applies a color and width picked on the pen wheel. */
    fun pickFromWheel(color: Int?, width: Double?) {
        val brush = settings.brush.copy(color = color ?: settings.brush.color, width = width ?: settings.brush.width)
        changeBrush(brush, colorChosen = settings.colorChosen || color != null)
        if (settings.tool == Tool.ERASER || settings.tool == Tool.SELECTOR) {
            selectTool(Tool.BRUSH)
        }
    }

    fun toggleShapeRecognition() {
        change(settings.copy(shapeRecognition = !settings.shapeRecognition))
        val enabled = settings.shapeRecognition
        val minSizePx = shapeMinSizePx()
        canvas.applyEdit { it.setShapeRecognition(enabled, minSizePx) }
    }

    /**
     * Keeps an automatic (not picked) brush color readable on [backgroundColor]: black or
     * white, whichever stands out.
     */
    fun adaptToBackground(backgroundColor: Int) {
        if (!settings.colorChosen) {
            val contrasting = BrushPalette.contrastingColor(backgroundColor)
            changeBrush(settings.brush.copy(color = contrasting), colorChosen = false)
        }
    }

    /**
     * Brings a document that just opened in line with the settings, as it starts with the
     * engine defaults. Queued before any pen input, so the first stroke already uses them.
     */
    fun applyToDocument() {
        val current = settings
        val minSizePx = shapeMinSizePx()
        renderer.setBrush(current.brush)
        canvas.applyEdit { document ->
            document.setShape(current.shape)
            document.setBrushStyle(current.brushStyle)
            document.setEraserStyle(current.eraserStyle)
            document.setShapeStyle(current.shapeStyle)
            // Only new text: nothing is being edited yet.
            document.setTextFormat(current.textFormat)
            document.setTool(current.tool)
            document.setShapeRecognition(current.shapeRecognition, minSizePx)
        }
    }

    private fun applyToView() {
        sidebar.selectedTool = settings.tool
        sidebar.showPalette(settings.palette, settings.brush.color)
        canvas.tool = settings.tool
        canvas.brush = settings.brush
        canvas.fingerDrawing = settings.fingerDrawing
        canvas.holdToSnap = settings.shapeRecognition
    }

    /** Strokes smaller than this on screen stay as drawn, even when the pen holds still. */
    private fun shapeMinSizePx(): Double = SHAPE_MIN_SIZE_DP * context.resources.displayMetrics.density.toDouble()

    private companion object {
        // Holding the pen still is what asks for a shape, so small ones are fine too.
        const val SHAPE_MIN_SIZE_DP = 24.0
    }
}
