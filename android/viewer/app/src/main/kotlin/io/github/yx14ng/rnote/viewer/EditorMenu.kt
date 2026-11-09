package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem

/** Everything the menu shows: what is open and the current editing choices. */
data class MenuState(val documentOpen: Boolean, val editState: EditState, val settings: EditorSettings)

/** Builds the options menu and maps its items back to actions. */
internal object EditorMenu {
    /** What a menu item does. */
    sealed interface Action {
        data object Undo : Action
        data object Redo : Action
        data class SelectColor(val color: Int) : Action
        data object CustomColor : Action
        data class SelectWidth(val width: Double) : Action
        data class SelectShape(val shape: Shape) : Action
        data class SelectBrushStyle(val style: BrushStyle) : Action
        data class SelectEraserStyle(val style: EraserStyle) : Action
        data class Selection(val action: SelectionAction) : Action
        data object Copy : Action
        data object Cut : Action
        data object Paste : Action
        data class Emphasis(val emphasis: TextEmphasis) : Action
        data object ColorSelectedText : Action
        data object SelectAllText : Action
        data object ToggleFingerDrawing : Action
        data object FinishShape : Action
        data object TextFormat : Action
        data object ShapeStyle : Action
        data object Document : Action
        data object New : Action
        data object Open : Action
        data object Save : Action
        data object SaveAs : Action
    }

    private const val UNDO = 1
    private const val REDO = 2
    private const val FINGER_DRAWING = 3
    private const val NEW = 4
    private const val OPEN = 5
    private const val SAVE = 6
    private const val SAVE_AS = 7
    private const val SELECT_ALL = 8
    private const val DESELECT_ALL = 9
    private const val DELETE_SELECTION = 10
    private const val DUPLICATE_SELECTION = 11
    private const val COLOR_MENU = 12
    private const val WIDTH_MENU = 13
    private const val SHAPE_MENU = 14
    private const val BRUSH_STYLE_MENU = 15
    private const val ERASER_MENU = 16
    private const val SELECTION_MENU = 17
    private const val FINISH_SHAPE = 18
    private const val TEXT_FORMAT = 19
    private const val SHAPE_STYLE = 20
    private const val DOCUMENT = 21
    private const val CUSTOM_COLOR = 22
    private const val COPY = 23
    private const val CUT = 24
    private const val PASTE = 25
    private const val TEXT_SELECTION_MENU = 26
    private const val COLOR_SELECTED_TEXT = 27
    private const val SELECT_ALL_TEXT = 28

    // Item ids of the choice groups are a base plus the index of the choice.
    private const val COLOR_FIRST = 200
    private const val WIDTH_FIRST = 300
    private const val SHAPE_FIRST = 400
    private const val BRUSH_STYLE_FIRST = 500
    private const val ERASER_FIRST = 600
    private const val EMPHASIS_FIRST = 700

    private const val GROUP_COLORS = 2
    private const val GROUP_WIDTHS = 3
    private const val GROUP_SHAPES = 4
    private const val GROUP_BRUSH_STYLES = 5
    private const val GROUP_ERASER_STYLES = 6

    // Ids of the items that need an open document.
    private val documentItems = listOf(
        COLOR_MENU, WIDTH_MENU, SHAPE_MENU, BRUSH_STYLE_MENU, ERASER_MENU, SELECTION_MENU,
        TEXT_FORMAT, SHAPE_STYLE, DOCUMENT, SAVE, SAVE_AS, COPY, CUT, PASTE,
    )

    // Ids of the items that act on the text being edited.
    private val textItems =
        listOf(COLOR_SELECTED_TEXT, SELECT_ALL_TEXT) + TextEmphasis.entries.indices.map { EMPHASIS_FIRST + it }

    private const val CTRL_SHIFT = KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON

    fun create(menu: Menu) {
        var order = 0
        menu.add(Menu.NONE, FINISH_SHAPE, order++, R.string.action_finish_shape)
            .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
        // Shortcuts use Ctrl unless told otherwise; they work with any hardware keyboard.
        menu.add(Menu.NONE, UNDO, order++, R.string.action_undo).apply {
            setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
            alphabeticShortcut = 'z'
        }
        menu.add(Menu.NONE, REDO, order++, R.string.action_redo).apply {
            setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
            setAlphabeticShortcut('z', CTRL_SHIFT)
        }

        addChoices(menu, COLOR_MENU, R.string.action_color, GROUP_COLORS, COLOR_FIRST, order++,
            BrushPalette.colors.map { it.label })
            .add(Menu.NONE, CUSTOM_COLOR, BrushPalette.colors.size, R.string.action_custom_color)
        addChoices(menu, WIDTH_MENU, R.string.action_width, GROUP_WIDTHS, WIDTH_FIRST, order++,
            BrushPalette.widths.map { it.label })
        addChoices(menu, BRUSH_STYLE_MENU, R.string.action_brush_style, GROUP_BRUSH_STYLES,
            BRUSH_STYLE_FIRST, order++, BrushStyle.entries.map { it.label })
        addChoices(menu, SHAPE_MENU, R.string.action_shape, GROUP_SHAPES, SHAPE_FIRST, order++,
            Shape.entries.map { it.label })
        menu.add(Menu.NONE, SHAPE_STYLE, order++, R.string.action_shape_style)
        menu.add(Menu.NONE, TEXT_FORMAT, order++, R.string.action_text_format)
        addChoices(menu, ERASER_MENU, R.string.action_eraser_mode, GROUP_ERASER_STYLES,
            ERASER_FIRST, order++, EraserStyle.entries.map { it.label })

        val selection = menu.addSubMenu(Menu.NONE, SELECTION_MENU, order++, R.string.action_selection)
        selection.add(Menu.NONE, SELECT_ALL, 0, R.string.action_select_all).alphabeticShortcut = 'a'
        selection.add(Menu.NONE, DESELECT_ALL, 1, R.string.action_deselect_all)
        selection.add(Menu.NONE, DUPLICATE_SELECTION, 2, R.string.action_duplicate_selection)
        selection.add(Menu.NONE, DELETE_SELECTION, 3, R.string.action_delete_selection)

        val text = menu.addSubMenu(Menu.NONE, TEXT_SELECTION_MENU, order++, R.string.action_text_selection)
        TextEmphasis.entries.forEachIndexed { index, emphasis ->
            text.add(Menu.NONE, EMPHASIS_FIRST + index, index, emphasis.label)
        }
        text.add(Menu.NONE, COLOR_SELECTED_TEXT, TextEmphasis.entries.size, R.string.action_color_selected_text)
        text.add(Menu.NONE, SELECT_ALL_TEXT, TextEmphasis.entries.size + 1, R.string.action_select_all_text)

        menu.add(Menu.NONE, CUT, order++, R.string.action_cut).alphabeticShortcut = 'x'
        menu.add(Menu.NONE, COPY, order++, R.string.action_copy).alphabeticShortcut = 'c'
        menu.add(Menu.NONE, PASTE, order++, R.string.action_paste).alphabeticShortcut = 'v'
        menu.add(Menu.NONE, FINGER_DRAWING, order++, R.string.action_finger_drawing).isCheckable = true
        menu.add(Menu.NONE, DOCUMENT, order++, R.string.action_document)
        menu.add(Menu.NONE, NEW, order++, R.string.action_new).alphabeticShortcut = 'n'
        menu.add(Menu.NONE, OPEN, order++, R.string.action_open).alphabeticShortcut = 'o'
        menu.add(Menu.NONE, SAVE, order++, R.string.action_save).alphabeticShortcut = 's'
        menu.add(Menu.NONE, SAVE_AS, order, R.string.action_save_as)
    }

    fun prepare(menu: Menu, state: MenuState, context: Context) {
        val settings = state.settings
        menu.findItem(UNDO).isEnabled = state.documentOpen && state.editState.canUndo
        menu.findItem(REDO).isEnabled = state.documentOpen && state.editState.canRedo
        for (id in documentItems) {
            menu.findItem(id).isEnabled = state.documentOpen
        }
        checkChoice(menu, COLOR_FIRST, BrushPalette.colors.indexOfFirst { it.value == settings.brush.color })
        checkChoice(menu, WIDTH_FIRST, BrushPalette.widths.indexOfFirst { it.value == settings.brush.width })
        checkChoice(menu, SHAPE_FIRST, settings.shape.ordinal)
        checkChoice(menu, BRUSH_STYLE_FIRST, settings.brushStyle.ordinal)
        checkChoice(menu, ERASER_FIRST, settings.eraserStyle.ordinal)
        val editingText = state.documentOpen && state.editState.textInput
        for (id in textItems) {
            menu.findItem(id).isEnabled = editingText
        }
        menu.findItem(DUPLICATE_SELECTION).isEnabled = state.editState.hasSelection
        menu.findItem(DELETE_SELECTION).isEnabled = state.editState.hasSelection
        menu.findItem(FINGER_DRAWING).isChecked = settings.fingerDrawing
        menu.findItem(FINISH_SHAPE).isVisible =
            state.documentOpen && settings.tool == Tool.SHAPER && settings.shape.needsFinish
    }

    /** The action for [item], or null for items that only open a submenu. */
    fun actionFor(item: MenuItem): Action? {
        val id = item.itemId
        return when (id) {
            UNDO -> Action.Undo
            REDO -> Action.Redo
            FINGER_DRAWING -> Action.ToggleFingerDrawing
            FINISH_SHAPE -> Action.FinishShape
            TEXT_FORMAT -> Action.TextFormat
            SHAPE_STYLE -> Action.ShapeStyle
            DOCUMENT -> Action.Document
            NEW -> Action.New
            OPEN -> Action.Open
            SAVE -> Action.Save
            SAVE_AS -> Action.SaveAs
            SELECT_ALL -> Action.Selection(SelectionAction.SELECT_ALL)
            DESELECT_ALL -> Action.Selection(SelectionAction.DESELECT_ALL)
            DELETE_SELECTION -> Action.Selection(SelectionAction.DELETE)
            DUPLICATE_SELECTION -> Action.Selection(SelectionAction.DUPLICATE)
            CUSTOM_COLOR -> Action.CustomColor
            COPY -> Action.Copy
            CUT -> Action.Cut
            PASTE -> Action.Paste
            COLOR_SELECTED_TEXT -> Action.ColorSelectedText
            SELECT_ALL_TEXT -> Action.SelectAllText
            else ->
                choice(id, COLOR_FIRST, BrushPalette.colors) { Action.SelectColor(it.value) }
                    ?: choice(id, WIDTH_FIRST, BrushPalette.widths) { Action.SelectWidth(it.value) }
                    ?: choice(id, SHAPE_FIRST, Shape.entries) { Action.SelectShape(it) }
                    ?: choice(id, BRUSH_STYLE_FIRST, BrushStyle.entries) { Action.SelectBrushStyle(it) }
                    ?: choice(id, ERASER_FIRST, EraserStyle.entries) { Action.SelectEraserStyle(it) }
                    ?: choice(id, EMPHASIS_FIRST, TextEmphasis.entries) { Action.Emphasis(it) }
        }
    }

    private fun addChoices(
        menu: Menu,
        submenuId: Int,
        submenuLabel: Int,
        group: Int,
        firstId: Int,
        order: Int,
        labels: List<Int>,
    ): Menu {
        val submenu = menu.addSubMenu(Menu.NONE, submenuId, order, submenuLabel)
        labels.forEachIndexed { index, label -> submenu.add(group, firstId + index, index, label) }
        submenu.setGroupCheckable(group, true, true)
        return submenu
    }

    private fun checkChoice(menu: Menu, firstId: Int, index: Int) {
        if (index >= 0) {
            menu.findItem(firstId + index).isChecked = true
        }
    }

    private fun <T> choice(id: Int, firstId: Int, choices: List<T>, action: (T) -> Action): Action? =
        choices.getOrNull(id - firstId)?.takeIf { id >= firstId }?.let(action)
}
