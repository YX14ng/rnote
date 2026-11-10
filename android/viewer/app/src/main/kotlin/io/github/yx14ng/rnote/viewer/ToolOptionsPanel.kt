package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.LinearLayout

/**
 * The options of a tool, in a panel beside the tool sidebar: opened by tapping the active
 * tool, or the width button for the pen. Each choice runs an [EditorMenu.Action] at once.
 */
internal object ToolOptionsPanel {
    /** Widths offered as presets; the pen wheel sets any other. */
    val PRESET_WIDTHS = listOf(1.0, 2.0, 3.0, 6.0, 10.0)

    fun build(context: Context, tool: Tool, settings: EditorSettings, onAction: (EditorMenu.Action) -> Unit): View {
        val panel = PanelViews.column(context, widthDp = PANEL_WIDTH_DP)
        when (tool) {
            Tool.BRUSH -> brush(context, panel, settings, onAction)
            Tool.SHAPER -> shapes(context, panel, settings, onAction)
            Tool.TYPEWRITER -> text(context, panel, onAction)
            Tool.ERASER -> eraser(context, panel, settings, onAction)
            Tool.SELECTOR -> lasso(context, panel, onAction)
        }
        return panel
    }

    private fun brush(context: Context, panel: LinearLayout, settings: EditorSettings, onAction: (EditorMenu.Action) -> Unit) {
        panel.addView(PanelViews.title(context, R.string.action_pen))
        panel.addView(PanelViews.label(context, R.string.label_style))
        panel.addView(PanelViews.segmented(
            context,
            BrushStyle.entries.map { context.getString(it.label) },
            settings.brushStyle.ordinal,
        ) { onAction(EditorMenu.Action.SelectBrushStyle(BrushStyle.entries[it])) })
        panel.addView(PanelViews.label(context, R.string.brush_width))
        val selected = PRESET_WIDTHS.indexOfFirst { it == settings.brush.width }
        panel.addView(PanelViews.segmented(
            context,
            PRESET_WIDTHS.map { ToolSidebar.formatWidth(it) },
            selected,
        ) { onAction(EditorMenu.Action.SelectWidth(PRESET_WIDTHS[it])) })
        panel.addView(StrokeSample(context, settings.brush), LinearLayout.LayoutParams(MATCH_PARENT, Graphite.dp(context, 56f)))
        panel.addView(PanelViews.divider(context), PanelViews.dividerParams(context))
        panel.addView(PanelViews.switchRow(context, R.string.shape_recognition_short, R.string.shape_recognition_hint,
            settings.shapeRecognition) { onAction(EditorMenu.Action.ToggleShapeRecognition) })
        panel.addView(PanelViews.switchRow(context, R.string.action_finger_drawing, null, settings.fingerDrawing) {
            onAction(EditorMenu.Action.ToggleFingerDrawing)
        })
    }

    private fun shapes(context: Context, panel: LinearLayout, settings: EditorSettings, onAction: (EditorMenu.Action) -> Unit) {
        panel.addView(PanelViews.title(context, R.string.action_shapes))
        panel.addView(PanelViews.grid(context, Shape.entries.map { context.getString(it.label) }, settings.shape.ordinal, 2) {
            onAction(EditorMenu.Action.SelectShape(Shape.entries[it]))
        }, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
        panel.addView(PanelViews.label(context, R.string.label_style))
        panel.addView(PanelViews.segmented(
            context,
            listOf(context.getString(R.string.shape_style_smooth), context.getString(R.string.shape_style_rough)),
            if (settings.shapeStyle.rough) 1 else 0,
        ) { onAction(EditorMenu.Action.ShapeRough(it == 1)) })
        panel.addView(PanelViews.switchRow(context, R.string.fill_with_pen_color, null, settings.shapeStyle.fill != null) {
            onAction(EditorMenu.Action.ShapeFill(settings.shapeStyle.fill == null))
        })
    }

    private fun text(context: Context, panel: LinearLayout, onAction: (EditorMenu.Action) -> Unit) {
        panel.addView(PanelViews.title(context, R.string.action_text))
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_text_format, R.string.action_text_format, null, true) {
            onAction(EditorMenu.Action.TextFormat)
        })
        panel.addView(PanelViews.label(context, R.string.action_text_selection))
        panel.addView(PanelViews.segmented(context, TextEmphasis.entries.map { context.getString(it.label) }, -1) {
            onAction(EditorMenu.Action.Emphasis(TextEmphasis.entries[it]))
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_palette, R.string.action_color_selected_text, null, true) {
            onAction(EditorMenu.Action.ColorSelectedText)
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_check, R.string.action_select_all_text, null, true) {
            onAction(EditorMenu.Action.SelectAllText)
        })
    }

    private fun eraser(context: Context, panel: LinearLayout, settings: EditorSettings, onAction: (EditorMenu.Action) -> Unit) {
        panel.addView(PanelViews.title(context, R.string.action_eraser))
        panel.addView(PanelViews.segmented(
            context,
            EraserStyle.entries.map { context.getString(it.label) },
            settings.eraserStyle.ordinal,
        ) { onAction(EditorMenu.Action.SelectEraserStyle(EraserStyle.entries[it])) })
    }

    private fun lasso(context: Context, panel: LinearLayout, onAction: (EditorMenu.Action) -> Unit) {
        panel.addView(PanelViews.title(context, R.string.action_select))
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_check, R.string.action_select_all, "Ctrl+A", true) {
            onAction(EditorMenu.Action.Selection(SelectionAction.SELECT_ALL))
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_close, R.string.action_deselect_all, null, true) {
            onAction(EditorMenu.Action.Selection(SelectionAction.DESELECT_ALL))
        })
    }

    /** A wavy line drawn with the brush's width, previewing it. */
    @SuppressLint("ViewConstructor") // Built in code only, with what it shows.
    private class StrokeSample(context: Context, brush: Brush) : View(context) {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            color = Graphite.TEXT
            strokeWidth = (brush.width.toFloat() * resources.displayMetrics.density * SAMPLE_SCALE).coerceAtLeast(1f)
        }

        private val path = Path()

        init {
            contentDescription = context.getString(R.string.stroke_preview)
        }

        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            val (x, y) = w.toFloat() to h.toFloat()
            path.reset()
            path.moveTo(x * 0.06f, y * 0.65f)
            path.cubicTo(x * 0.3f, y * 0.05f, x * 0.45f, y * 0.95f, x * 0.65f, y * 0.45f)
            path.cubicTo(x * 0.78f, y * 0.15f, x * 0.88f, y * 0.55f, x * 0.94f, y * 0.4f)
        }

        override fun onDraw(canvas: Canvas) {
            canvas.drawPath(path, paint)
        }

        private companion object {
            /** A document unit at about the zoom of a page fitted to a tablet's width. */
            const val SAMPLE_SCALE = 0.8f
        }
    }

    private const val PANEL_WIDTH_DP = 340f
}
