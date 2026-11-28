package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.graphics.Point
import android.view.HapticFeedbackConstants

/**
 * What gestures on the page open or do: the page's menu where a finger holds still, the pen
 * wheel around the S Pen, and undo and redo with two- and three-finger taps.
 */
internal class CanvasGestures(
    private val activity: Activity,
    private val canvas: NoteCanvasView,
    private val layout: () -> EditorLayout,
    private val controls: () -> EditorControls,
    /** The editing state of the open notebook; null when none is open. */
    private val state: () -> EditState?,
    private val perform: (EditorMenu.Action) -> Unit,
    private val refreshSelectionBar: () -> Unit,
) {
    /**
     * Opens the page's menu where a finger held still: at ([x], [y]) on the canvas, and
     * ([documentX], [documentY]) in the document, where pasting puts things.
     */
    fun showCanvasMenu(x: Float, y: Float, documentX: Double, documentY: Double) {
        val current = state() ?: return
        val entries = CanvasMenu.entries(current.hasSelection, documentX, documentY)
        val panel = CanvasMenu.build(activity, entries, perform)
        val canvasAt = IntArray(2).also(canvas::getLocationInWindow)
        val point = Point(canvasAt[0] + x.toInt(), canvasAt[1] + y.toInt())
        val dismiss = {
            layout().hideOverlay()
            refreshSelectionBar()
        }
        layout().showOverlay(PanelOverlay(activity, panel, CanvasMenu.PANEL_WIDTH_DP, null, dismiss, point))
    }

    /** Opens the wheel to pick the brush color and width around the pen at ([x], [y]). */
    fun showPenWheel(x: Float, y: Float) {
        if (state() == null) return
        val controls = controls()
        val settings = controls.settings
        val wheel = PenWheel(
            activity,
            colors = controls.paletteOnPaper,
            presets = ToolOptionsPanel.PRESET_WIDTHS,
            color = settings.brush.color,
            brushWidth = settings.brush.width,
            onPick = controls::pickFromWheel,
            onClose = layout()::hideOverlay,
        )
        wheel.centerAt(x, y)
        layout().showOverlay(wheel)
    }

    /** A two-finger tap undoes, like in other note-taking apps. */
    fun undo() {
        if (state()?.canUndo == true) {
            canvas.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
            canvas.applyEdit(NativeDocument::undo)
        }
    }

    /** A three-finger tap redoes, like in other note-taking apps. */
    fun redo() {
        if (state()?.canRedo == true) {
            canvas.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
            canvas.applyEdit(NativeDocument::redo)
        }
    }
}
