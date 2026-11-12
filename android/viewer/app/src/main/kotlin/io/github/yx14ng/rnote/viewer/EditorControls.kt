package io.github.yx14ng.rnote.viewer

import android.content.Context

/**
 * The editing choices (tool, brush, shapes, text, ...): kept in [EditorSettings] across
 * restarts, shown by the canvas and the tool row, and applied to the open document.
 *
 * [onChanged] runs after every change, e.g. to refresh the menu.
 */
internal class EditorControls(
    private val context: Context,
    private val canvas: NoteCanvasView,
    private val sidebar: ToolRow,
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

    /** Sets the palette color in [slot] to [color], which becomes the pen's at once. */
    fun setPaletteColor(slot: Int, color: Int) {
        if (slot !in settings.palette.indices) return
        val palette = settings.palette.toMutableList().also { it[slot] = color }
        change(settings.copy(palette = palette))
        selectPaletteColor(slot)
    }

    /** Remembers the colors kept in the color menu. */
    fun keepColors(colors: List<Int>) = change(settings.copy(savedColors = colors))


    /** Applies a color and width picked on the pen wheel. */
    fun pickFromWheel(color: Int?, width: Double?) {
        val brush = settings.brush.copy(color = color ?: settings.brush.color, width = width ?: settings.brush.width)
        changeBrush(brush, colorChosen = settings.colorChosen || color != null)
        if (settings.tool == Tool.ERASER || settings.tool == Tool.SELECTOR) {
            selectTool(Tool.BRUSH)
        }
    }

    /**
     * Runs the editing [action]s: tools, styles, selection and text formatting. Returns false
     * for the ones it leaves to the activity (files, clipboard, document, zoom, panels).
     */
    fun perform(action: EditorMenu.Action): Boolean {
        val current = settings
        when (action) {
            EditorMenu.Action.Undo -> canvas.applyEdit(NativeDocument::undo)
            EditorMenu.Action.Redo -> canvas.applyEdit(NativeDocument::redo)
            EditorMenu.Action.FinishShape -> canvas.finishShape()
            is EditorMenu.Action.SelectWidth ->
                changeBrush(current.brush.copy(width = action.width), current.colorChosen)
            is EditorMenu.Action.SelectShape -> {
                // Picking a shape means drawing it next.
                change(current.copy(shape = action.shape, tool = Tool.SHAPER))
                canvas.applyEdit { it.setShape(action.shape); it.setTool(Tool.SHAPER) }
            }
            is EditorMenu.Action.SelectBrushStyle -> {
                change(current.copy(brushStyle = action.style))
                canvas.applyEdit { it.setBrushStyle(action.style) }
            }
            is EditorMenu.Action.SelectEraserStyle -> {
                change(current.copy(eraserStyle = action.style))
                canvas.applyEdit { it.setEraserStyle(action.style) }
            }
            is EditorMenu.Action.ShapeRough -> changeShapeStyle(current.shapeStyle.copy(rough = action.rough))
            is EditorMenu.Action.ShapeFill ->
                changeShapeStyle(current.shapeStyle.copy(fill = if (action.filled) current.brush.color else null))
            is EditorMenu.Action.Selection -> {
                if (action.action == SelectionAction.SELECT_ALL) {
                    // The engine switches to the selector to show the selection.
                    change(current.copy(tool = Tool.SELECTOR))
                }
                canvas.applyEdit { it.selection(action.action) }
            }
            is EditorMenu.Action.RecolorSelection -> canvas.applyEdit { it.recolorSelection(action.color) }
            is EditorMenu.Action.Emphasis -> canvas.applyEdit { it.toggleTextEmphasis(action.emphasis) }
            EditorMenu.Action.ColorSelectedText -> canvas.applyEdit { it.colorSelectedText(current.brush.color) }
            EditorMenu.Action.SelectAllText ->
                canvas.applyEdit { it.handleCharKey('a'.code, shift = false, ctrl = true) }
            EditorMenu.Action.TextFormat -> FormatDialogs.showTextFormat(context, current.textFormat) { format ->
                change(settings.copy(textFormat = format))
                canvas.applyEdit { it.setTextFormat(format) }
            }
            EditorMenu.Action.ToggleFingerDrawing -> change(current.copy(fingerDrawing = !current.fingerDrawing))
            EditorMenu.Action.ToggleShapeRecognition -> toggleShapeRecognition()
            else -> return false
        }
        return true
    }

    private fun changeShapeStyle(style: ShapeStyle) {
        change(settings.copy(shapeStyle = style))
        canvas.applyEdit { it.setShapeStyle(style) }
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
        sidebar.showWidth(settings.brush.width)
        canvas.tool = settings.tool
        canvas.brush = settings.brush
        canvas.brushStyle = settings.brushStyle
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
