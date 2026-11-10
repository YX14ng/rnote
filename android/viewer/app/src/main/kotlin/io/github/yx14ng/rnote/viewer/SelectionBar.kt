package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout

/**
 * The actions for what the lasso selected, floating above the selection (below it when there
 * is no room): copy, cut, duplicate, color and delete. "Color" shows the palette's colors to
 * paint the selection with.
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class SelectionBar(
    context: Context,
    private val palette: () -> List<Int>,
    private val onAction: (EditorMenu.Action) -> Unit,
) : LinearLayout(context) {
    private val actions = LinearLayout(context).apply { orientation = HORIZONTAL }
    private val colors = LinearLayout(context).apply {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER
        visibility = GONE
    }

    // The selection, in document coordinates, while the bar shows.
    private var selection: DocumentBounds? = null
    private var viewport = Viewport(0.0, 0.0, 1.0)

    init {
        orientation = VERTICAL
        background = Graphite.rounded(context, Graphite.BAR, 18f, Graphite.OUTLINE)
        elevation = Graphite.dp(context, 10f).toFloat()
        val pad = Graphite.dp(context, 6f)
        setPadding(pad, pad, pad, pad)
        isClickable = true
        visibility = GONE
        for ((icon, label, action) in listOf(
            Triple(R.drawable.ic_copy, R.string.action_copy, EditorMenu.Action.Copy),
            Triple(R.drawable.ic_cut, R.string.action_cut, EditorMenu.Action.Cut),
            Triple(R.drawable.ic_duplicate, R.string.action_duplicate_selection, EditorMenu.Action.Selection(SelectionAction.DUPLICATE)),
        )) {
            actions.addView(PanelViews.bigButton(context, icon, label) { onAction(action) })
        }
        actions.addView(PanelViews.bigButton(context, R.drawable.ic_palette, R.string.recolor_selection) { toggleColors() })
        actions.addView(PanelViews.bigButton(context, R.drawable.ic_delete, R.string.action_delete_selection, Graphite.DANGER) {
            onAction(EditorMenu.Action.Selection(SelectionAction.DELETE))
        })
        addView(actions)
        addView(colors)
    }

    /** Shows the bar for [bounds] (document coordinates), or hides it when null. */
    fun show(bounds: DocumentBounds?, currentViewport: Viewport) {
        selection = bounds
        viewport = currentViewport
        if (bounds == null) {
            hide()
            return
        }
        visibility = VISIBLE
        place()
    }

    /** Follows the selection when the view pans or zooms. */
    fun follow(currentViewport: Viewport) {
        viewport = currentViewport
        if (visibility == VISIBLE) place()
    }

    fun hide() {
        visibility = GONE
        colors.visibility = GONE
    }

    private fun toggleColors() {
        if (colors.visibility == VISIBLE) {
            colors.visibility = GONE
            return
        }
        colors.removeAllViews()
        val size = Graphite.dp(context, 32f)
        val gap = Graphite.dp(context, 6f)
        for (color in palette()) {
            colors.addView(View(context).apply {
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(color)
                    setStroke(Graphite.dp(context, 1f), Graphite.OUTLINE)
                }
                contentDescription = context.getString(R.string.recolor_selection)
                setOnClickListener { onAction(EditorMenu.Action.RecolorSelection(color)) }
            }, LayoutParams(size, size).apply { setMargins(gap, gap, gap, gap) })
        }
        colors.visibility = VISIBLE
        post(::place)
    }

    /** Puts the bar above the selection on screen, or below it, within the parent. */
    private fun place() {
        val bounds = selection ?: return
        val parent = parent as? View ?: return
        if (width == 0 || height == 0) {
            // Not measured yet: place it once it is.
            post(::place)
            return
        }
        val z = viewport.zoom
        val left = ((bounds.left - viewport.x) * z).toFloat()
        val right = ((bounds.right - viewport.x) * z).toFloat()
        val top = ((bounds.top - viewport.y) * z).toFloat()
        val bottom = ((bounds.bottom - viewport.y) * z).toFloat()
        val margin = Graphite.dp(context, 12f).toFloat()
        val availableWidth = (parent.width - parent.paddingLeft - parent.paddingRight).toFloat()
        val availableHeight = (parent.height - parent.paddingTop - parent.paddingBottom).toFloat()
        translationX = ((left + right) / 2 - width / 2).coerceIn(margin, maxOf(margin, availableWidth - width - margin))
        val above = top - height - margin
        translationY = if (above >= margin) above else (bottom + margin).coerceAtMost(availableHeight - height - margin)
    }
}
