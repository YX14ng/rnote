package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView

/**
 * The editor's top bar, in two rows like other note-taking apps. The first has back to the
 * library, the notebook's name and the document actions; the second, which folds away, has
 * undo and redo, the tools in the middle and the palette at the end.
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class EditorTopBar(
    context: Context,
    tools: ToolRow,
    private val onAction: (EditorMenu.Action) -> Unit,
    onBack: () -> Unit,
    onTitle: () -> Unit,
) : LinearLayout(context) {
    private val title = TextView(context).apply {
        textSize = 17f
        setTextColor(Graphite.TEXT)
        maxLines = 1
        ellipsize = TextUtils.TruncateAt.MIDDLE
    }
    private val unsaved = TextView(context).apply {
        textSize = 12f
        setTextColor(Graphite.TEXT_SECONDARY)
        setText(R.string.unsaved)
    }
    private val finishShape = button(R.drawable.ic_check, R.string.action_finish_shape) { onAction(EditorMenu.Action.FinishShape) }
    private val zoom = TextView(context).apply {
        textSize = 14f
        setTextColor(Graphite.TEXT)
        gravity = Gravity.CENTER
        background = Graphite.pressable(context, null, Graphite.BUTTON_RADIUS_DP)
        contentDescription = context.getString(R.string.action_zoom_fit)
        tooltipText = contentDescription
        setOnClickListener { onAction(EditorMenu.Action.ZoomToFit) }
    }
    private val undo = button(R.drawable.ic_undo, R.string.action_undo) { onAction(EditorMenu.Action.Undo) }
    private val redo = button(R.drawable.ic_redo, R.string.action_redo) { onAction(EditorMenu.Action.Redo) }
    private val fold = button(R.drawable.ic_chevron_down, R.string.toolbar_fold) { toggleToolsRow() }

    /** Where the menu opens. */
    val moreButton: View = button(R.drawable.ic_more, R.string.more_options) { onAction(EditorMenu.Action.More) }

    private val toolsRow = FrameLayout(context).apply {
        setPadding(dp(8f), 0, dp(8f), 0)
        addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(undo, square())
            addView(redo, square())
        }, FrameLayout.LayoutParams(WRAP_CONTENT, MATCH_PARENT, Gravity.START))
        addView(tools.tools, FrameLayout.LayoutParams(WRAP_CONTENT, MATCH_PARENT, Gravity.CENTER))
        addView(tools.palette, FrameLayout.LayoutParams(WRAP_CONTENT, MATCH_PARENT, Gravity.END))
    }

    init {
        orientation = VERTICAL
        setBackgroundColor(Graphite.BAR)
        elevation = dp(4f).toFloat()
        val titles = LinearLayout(context).apply {
            orientation = VERTICAL
            gravity = Gravity.CENTER_VERTICAL
            background = Graphite.pressable(context, null, Graphite.BUTTON_RADIUS_DP)
            setPadding(dp(8f), 0, dp(8f), 0)
            setOnClickListener { onTitle() }
            addView(title)
            addView(unsaved)
        }
        val firstRow = LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(4f), 0, dp(8f), 0)
            addView(button(R.drawable.ic_back, R.string.back_to_library) { onBack() }, square())
            addView(titles, LayoutParams(0, MATCH_PARENT, 1f).apply { marginStart = dp(4f) })
            addView(finishShape, square())
            addView(zoom, LayoutParams(dp(72f), dp(44f)))
            addView(moreButton, square())
            addView(fold, square())
        }
        addView(firstRow, LayoutParams(MATCH_PARENT, dp(FIRST_ROW_DP)))
        addView(View(context).apply { setBackgroundColor(Graphite.DIVIDER) }, LayoutParams(MATCH_PARENT, dp(1f)))
        addView(toolsRow, LayoutParams(MATCH_PARENT, dp(TOOLS_ROW_DP)))
        // Pointing up: tapping folds the tools row away.
        fold.rotation = 180f
    }

    /** Shows the notebook's [name] and the state of the document. */
    fun show(name: String, state: MenuState) {
        title.text = name
        unsaved.visibility = if (state.editState.modified) VISIBLE else GONE
        enable(undo, state.documentOpen && state.editState.canUndo)
        enable(redo, state.documentOpen && state.editState.canRedo)
        zoom.text = context.getString(R.string.zoom_percent, state.zoomPercent)
        zoom.visibility = if (state.documentOpen) VISIBLE else GONE
        finishShape.visibility =
            if (state.documentOpen && state.settings.tool == Tool.SHAPER && state.settings.shape.needsFinish) VISIBLE else GONE
    }

    private fun toggleToolsRow() {
        val folding = toolsRow.visibility == VISIBLE
        toolsRow.visibility = if (folding) GONE else VISIBLE
        fold.rotation = if (folding) 0f else 180f
        fold.contentDescription = context.getString(if (folding) R.string.toolbar_unfold else R.string.toolbar_fold)
        fold.tooltipText = fold.contentDescription
    }

    private fun enable(button: View, enabled: Boolean) {
        button.isEnabled = enabled
        button.alpha = if (enabled) 1f else DISABLED_ALPHA
    }

    private fun button(icon: Int, label: Int, onClick: () -> Unit): ImageButton =
        Graphite.iconButton(context, icon, label, onClick)

    private fun square() = LayoutParams(dp(BUTTON_DP), dp(BUTTON_DP))

    private fun dp(value: Float): Int = Graphite.dp(context, value)

    private companion object {
        const val FIRST_ROW_DP = 52f
        const val TOOLS_ROW_DP = 56f
        const val BUTTON_DP = 44f
        const val DISABLED_ALPHA = 0.35f
    }
}
