package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.view.View

/**
 * The menu that opens where a finger holds still on the page: paste, or put a picture or a
 * photo, there; and what the lasso can do with the selection, or select everything when
 * nothing is selected.
 */
internal object CanvasMenu {
    /** One row of the menu; [danger] marks what can't be taken back without undo. */
    data class Entry(val icon: Int, val label: Int, val action: EditorMenu.Action, val danger: Boolean = false)

    /** The rows for a hold at ([x], [y]) in document coordinates. */
    fun entries(hasSelection: Boolean, x: Double, y: Double): List<Entry> {
        val paste = Entry(R.drawable.ic_paste, R.string.action_paste, EditorMenu.Action.PasteAt(x, y))
        if (!hasSelection) {
            return listOf(
                paste,
                Entry(R.drawable.ic_image, R.string.action_insert_image, EditorMenu.Action.InsertImageAt(x, y)),
                Entry(R.drawable.ic_camera, R.string.action_take_photo, EditorMenu.Action.TakePhotoAt(x, y)),
                Entry(R.drawable.ic_pen_lasso, R.string.action_select_all, EditorMenu.Action.Selection(SelectionAction.SELECT_ALL)),
            )
        }
        return listOf(
            paste,
            Entry(R.drawable.ic_copy, R.string.action_copy, EditorMenu.Action.Copy),
            Entry(R.drawable.ic_cut, R.string.action_cut, EditorMenu.Action.Cut),
            Entry(R.drawable.ic_duplicate, R.string.action_duplicate_selection, EditorMenu.Action.Selection(SelectionAction.DUPLICATE)),
            Entry(R.drawable.ic_delete, R.string.action_delete_selection, EditorMenu.Action.Selection(SelectionAction.DELETE), danger = true),
        )
    }

    fun build(context: Context, entries: List<Entry>, onAction: (EditorMenu.Action) -> Unit): View {
        val panel = PanelViews.column(context)
        for (entry in entries) {
            val tint = if (entry.danger) Graphite.DANGER else Graphite.TEXT
            panel.addView(PanelViews.menuRow(context, entry.icon, entry.label, null, true, tint) { onAction(entry.action) })
        }
        return panel
    }

    /** How wide the menu is. */
    const val PANEL_WIDTH_DP = 220f
}
