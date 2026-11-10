package io.github.yx14ng.rnote.viewer

import android.view.KeyEvent

/** The editor's keyboard shortcuts, as on the desktop app. */
internal object KeyShortcuts {
    /** The action for [keyCode] pressed with Ctrl (and [shift]), if it has one. */
    fun actionFor(keyCode: Int, ctrl: Boolean, shift: Boolean): EditorMenu.Action? {
        if (!ctrl) {
            return null
        }
        return when (keyCode) {
            KeyEvent.KEYCODE_Z -> if (shift) EditorMenu.Action.Redo else EditorMenu.Action.Undo
            KeyEvent.KEYCODE_Y -> EditorMenu.Action.Redo
            KeyEvent.KEYCODE_X -> EditorMenu.Action.Cut
            KeyEvent.KEYCODE_C -> EditorMenu.Action.Copy
            KeyEvent.KEYCODE_V -> EditorMenu.Action.Paste
            KeyEvent.KEYCODE_A -> EditorMenu.Action.Selection(SelectionAction.SELECT_ALL)
            KeyEvent.KEYCODE_N -> EditorMenu.Action.New
            KeyEvent.KEYCODE_O -> EditorMenu.Action.Open
            KeyEvent.KEYCODE_S -> EditorMenu.Action.Save
            else -> null
        }
    }
}
