package io.github.yx14ng.rnote.viewer

import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem

/** Everything the top bar shows: what is open, the editing state and the zoom. */
data class MenuState(
    val documentOpen: Boolean,
    val editState: EditState,
    val settings: EditorSettings,
    /** The zoom relative to fitting a page to the width, in percent. */
    val zoomPercent: Int,
)

/**
 * The top bar: icons for finishing a shape, undo, redo, the zoom (tapping it fits the page
 * width again) and the menu, which [Action.More] opens as a panel.
 *
 * Hidden items keep the hardware keyboard shortcuts working: Android looks for shortcuts in
 * hidden items too.
 */
internal object EditorMenu {
    /** Something the user asked for, from the top bar, a panel or a shortcut. */
    sealed interface Action {
        data object Undo : Action
        data object Redo : Action
        data object ZoomToFit : Action
        data object More : Action
        data object FinishShape : Action
        data class SelectWidth(val width: Double) : Action
        data class SelectShape(val shape: Shape) : Action
        data class SelectBrushStyle(val style: BrushStyle) : Action
        data class SelectEraserStyle(val style: EraserStyle) : Action
        data class Selection(val action: SelectionAction) : Action
        data class RecolorSelection(val color: Int) : Action
        data object Copy : Action
        data object Cut : Action
        data object Paste : Action
        data class Emphasis(val emphasis: TextEmphasis) : Action
        data object ColorSelectedText : Action
        data object SelectAllText : Action
        data object ToggleFingerDrawing : Action
        data object ToggleShapeRecognition : Action
        data class ShapeFill(val filled: Boolean) : Action
        data class ShapeRough(val rough: Boolean) : Action
        data object TextFormat : Action
        data object Document : Action
        data object New : Action
        data object Open : Action
        data object Save : Action
        data object SaveAs : Action
    }

    private const val FINISH_SHAPE = 1
    private const val UNDO = 2
    private const val REDO = 3
    private const val ZOOM = 4
    private const val MORE = 5
    private const val CUT = 10
    private const val COPY = 11
    private const val PASTE = 12
    private const val SELECT_ALL = 13
    private const val NEW = 14
    private const val OPEN = 15
    private const val SAVE = 16

    private const val CTRL_SHIFT = KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON
    private const val DISABLED_ALPHA = 90
    private const val ENABLED_ALPHA = 255

    // Items that need an open document.
    private val documentItems = listOf(ZOOM, CUT, COPY, PASTE, SELECT_ALL, SAVE)

    fun create(menu: Menu) {
        var order = 0
        fun icon(id: Int, label: Int, icon: Int) = menu.add(Menu.NONE, id, order++, label).apply {
            setIcon(icon)
            setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        }
        icon(FINISH_SHAPE, R.string.action_finish_shape, R.drawable.ic_check)
        // Shortcuts use Ctrl unless told otherwise.
        icon(UNDO, R.string.action_undo, R.drawable.ic_undo).alphabeticShortcut = 'z'
        icon(REDO, R.string.action_redo, R.drawable.ic_redo).setAlphabeticShortcut('z', CTRL_SHIFT)
        menu.add(Menu.NONE, ZOOM, order++, "100 %").setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        icon(MORE, R.string.more_options, R.drawable.ic_more)
        for ((id, label, key) in listOf(
            Triple(CUT, R.string.action_cut, 'x'),
            Triple(COPY, R.string.action_copy, 'c'),
            Triple(PASTE, R.string.action_paste, 'v'),
            Triple(SELECT_ALL, R.string.action_select_all, 'a'),
            Triple(NEW, R.string.action_new, 'n'),
            Triple(OPEN, R.string.action_open, 'o'),
            Triple(SAVE, R.string.action_save, 's'),
        )) {
            menu.add(Menu.NONE, id, order++, label).apply {
                alphabeticShortcut = key
                isVisible = false
            }
        }
    }

    fun prepare(menu: Menu, state: MenuState) {
        fun enable(id: Int, enabled: Boolean) = menu.findItem(id).apply {
            isEnabled = enabled
            icon?.alpha = if (enabled) ENABLED_ALPHA else DISABLED_ALPHA
        }
        enable(UNDO, state.documentOpen && state.editState.canUndo)
        enable(REDO, state.documentOpen && state.editState.canRedo)
        for (id in documentItems) {
            menu.findItem(id).isEnabled = state.documentOpen
        }
        menu.findItem(ZOOM).apply {
            isVisible = state.documentOpen
            title = "${state.zoomPercent} %"
        }
        menu.findItem(FINISH_SHAPE).isVisible =
            state.documentOpen && state.settings.tool == Tool.SHAPER && state.settings.shape.needsFinish
    }

    /** The action for [item], if it has one. */
    fun actionFor(item: MenuItem): Action? =
        when (item.itemId) {
            FINISH_SHAPE -> Action.FinishShape
            UNDO -> Action.Undo
            REDO -> Action.Redo
            ZOOM -> Action.ZoomToFit
            MORE -> Action.More
            CUT -> Action.Cut
            COPY -> Action.Copy
            PASTE -> Action.Paste
            SELECT_ALL -> Action.Selection(SelectionAction.SELECT_ALL)
            NEW -> Action.New
            OPEN -> Action.Open
            SAVE -> Action.Save
            else -> null
        }
}
