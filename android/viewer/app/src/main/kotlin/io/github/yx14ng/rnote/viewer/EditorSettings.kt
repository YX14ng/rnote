package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.graphics.Color

/** The editing choices, kept across documents and app restarts. */
data class EditorSettings(
    val tool: Tool = Tool.BRUSH,
    val brush: Brush = Brush(Color.BLACK, BrushPalette.DEFAULT_WIDTH),
    /** Until the user picks a color, it follows the document background. */
    val colorChosen: Boolean = false,
    val shape: Shape = Shape.LINE,
    val brushStyle: BrushStyle = BrushStyle.SOLID,
    val eraserStyle: EraserStyle = EraserStyle.WHOLE_STROKES,
    val fingerDrawing: Boolean = false,
    /** Whether brush strokes that look like shapes become those shapes. */
    val shapeRecognition: Boolean = false,
    val textFormat: TextFormat = TextFormat(),
    val shapeStyle: ShapeStyle = ShapeStyle(),
    /** The colors in the tool sidebar, which the user can change. */
    val palette: List<Int> = BrushPalette.DEFAULT_SIDEBAR_COLORS,
) {
    fun save(context: Context) {
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit()
            .putString(KEY_TOOL, tool.name)
            .putInt(KEY_COLOR, brush.color)
            .putFloat(KEY_WIDTH, brush.width.toFloat())
            .putBoolean(KEY_COLOR_CHOSEN, colorChosen)
            .putString(KEY_SHAPE, shape.name)
            .putString(KEY_BRUSH_STYLE, brushStyle.name)
            .putString(KEY_ERASER_STYLE, eraserStyle.name)
            .putBoolean(KEY_FINGER_DRAWING, fingerDrawing)
            .putString(KEY_PALETTE, palette.joinToString(","))
            .putBoolean(KEY_SHAPE_RECOGNITION, shapeRecognition)
            .putString(KEY_FONT_FAMILY, textFormat.fontFamily.name)
            .putFloat(KEY_FONT_SIZE, textFormat.fontSize.toFloat())
            .putBoolean(KEY_BOLD, textFormat.bold)
            .putBoolean(KEY_ITALIC, textFormat.italic)
            .putString(KEY_ALIGNMENT, textFormat.alignment.name)
            .putBoolean(KEY_ROUGH, shapeStyle.rough)
            .putBoolean(KEY_FILLED, shapeStyle.fill != null)
            .putInt(KEY_FILL, shapeStyle.fill ?: 0)
            .apply()
    }

    companion object {
        private const val PREFERENCES = "editor_settings"
        private const val KEY_TOOL = "tool"
        private const val KEY_COLOR = "color"
        private const val KEY_WIDTH = "width"
        private const val KEY_COLOR_CHOSEN = "color_chosen"
        private const val KEY_SHAPE = "shape"
        private const val KEY_BRUSH_STYLE = "brush_style"
        private const val KEY_ERASER_STYLE = "eraser_style"
        private const val KEY_FINGER_DRAWING = "finger_drawing"
        private const val KEY_SHAPE_RECOGNITION = "shape_recognition"
        private const val KEY_FONT_FAMILY = "font_family"
        private const val KEY_PALETTE = "palette"
        private const val KEY_FONT_SIZE = "font_size"
        private const val KEY_BOLD = "bold"
        private const val KEY_ITALIC = "italic"
        private const val KEY_ALIGNMENT = "alignment"
        private const val KEY_ROUGH = "rough"
        private const val KEY_FILLED = "filled"
        private const val KEY_FILL = "fill"

        /** The saved settings, falling back to the defaults for anything missing or unknown. */
        fun load(context: Context): EditorSettings {
            val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            val defaults = EditorSettings()
            return EditorSettings(
                tool = enumOrDefault(preferences.getString(KEY_TOOL, null), defaults.tool),
                brush = Brush(
                    preferences.getInt(KEY_COLOR, defaults.brush.color),
                    preferences.getFloat(KEY_WIDTH, defaults.brush.width.toFloat()).toDouble(),
                ),
                colorChosen = preferences.getBoolean(KEY_COLOR_CHOSEN, defaults.colorChosen),
                shape = enumOrDefault(preferences.getString(KEY_SHAPE, null), defaults.shape),
                brushStyle = enumOrDefault(preferences.getString(KEY_BRUSH_STYLE, null), defaults.brushStyle),
                eraserStyle = enumOrDefault(preferences.getString(KEY_ERASER_STYLE, null), defaults.eraserStyle),
                fingerDrawing = preferences.getBoolean(KEY_FINGER_DRAWING, defaults.fingerDrawing),
                shapeRecognition = preferences.getBoolean(KEY_SHAPE_RECOGNITION, defaults.shapeRecognition),
                textFormat = TextFormat(
                    fontFamily = enumOrDefault(
                        preferences.getString(KEY_FONT_FAMILY, null),
                        defaults.textFormat.fontFamily,
                    ),
                    fontSize = preferences.getFloat(KEY_FONT_SIZE, defaults.textFormat.fontSize.toFloat()).toDouble(),
                    bold = preferences.getBoolean(KEY_BOLD, defaults.textFormat.bold),
                    italic = preferences.getBoolean(KEY_ITALIC, defaults.textFormat.italic),
                    alignment = enumOrDefault(
                        preferences.getString(KEY_ALIGNMENT, null),
                        defaults.textFormat.alignment,
                    ),
                ),
                shapeStyle = ShapeStyle(
                    rough = preferences.getBoolean(KEY_ROUGH, defaults.shapeStyle.rough),
                    fill = if (preferences.getBoolean(KEY_FILLED, false)) preferences.getInt(KEY_FILL, 0) else null,
                ),
                palette = parsePalette(preferences.getString(KEY_PALETTE, null)) ?: defaults.palette,
            )
        }

        /** Colors saved as comma-separated ints, if there are as many as the palette holds. */
        private fun parsePalette(saved: String?): List<Int>? {
            val colors = saved?.split(",")?.map { it.toIntOrNull() ?: return null } ?: return null
            return colors.takeIf { it.size == BrushPalette.DEFAULT_SIDEBAR_COLORS.size }
        }

        private inline fun <reified T : Enum<T>> enumOrDefault(name: String?, default: T): T =
            enumValues<T>().firstOrNull { it.name == name } ?: default
    }
}
