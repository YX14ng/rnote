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
            )
        }

        private inline fun <reified T : Enum<T>> enumOrDefault(name: String?, default: T): T =
            enumValues<T>().firstOrNull { it.name == name } ?: default
    }
}
