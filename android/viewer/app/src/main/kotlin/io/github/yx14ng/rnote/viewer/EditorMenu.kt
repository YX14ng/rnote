package io.github.yx14ng.rnote.viewer

/** Everything the top bar and the menu show: what is open, the editing state and the zoom. */
data class MenuState(
    val documentOpen: Boolean,
    val editState: EditState,
    val settings: EditorSettings,
    /** The zoom relative to fitting a page to the width, in percent. */
    val zoomPercent: Int,
)

/** What the user can ask the editor for, from the top bar, a panel or a shortcut. */
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
}
