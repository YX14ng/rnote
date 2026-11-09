package io.github.yx14ng.rnote.viewer

import android.view.KeyEvent

/** Translates Android key events, from hardware or soft keyboards, for the text tool. */
internal object KeyMapping {
    private val editingKeys = mapOf(
        KeyEvent.KEYCODE_DEL to EditingKey.BACKSPACE,
        KeyEvent.KEYCODE_FORWARD_DEL to EditingKey.DELETE,
        KeyEvent.KEYCODE_ENTER to EditingKey.ENTER,
        KeyEvent.KEYCODE_NUMPAD_ENTER to EditingKey.ENTER,
        KeyEvent.KEYCODE_ESCAPE to EditingKey.ESCAPE,
        KeyEvent.KEYCODE_DPAD_LEFT to EditingKey.LEFT,
        KeyEvent.KEYCODE_DPAD_RIGHT to EditingKey.RIGHT,
        KeyEvent.KEYCODE_DPAD_UP to EditingKey.UP,
        KeyEvent.KEYCODE_DPAD_DOWN to EditingKey.DOWN,
        KeyEvent.KEYCODE_MOVE_HOME to EditingKey.HOME,
        KeyEvent.KEYCODE_MOVE_END to EditingKey.END,
        KeyEvent.KEYCODE_TAB to EditingKey.TAB,
    )

    /**
     * Sends [event] to [sink]. Returns false for keys that mean nothing to the text tool,
     * including Ctrl shortcuts other than Ctrl+A, which are left to the menu.
     */
    fun dispatch(event: KeyEvent, sink: TextSink): Boolean {
        val editingKey = editingKeys[event.keyCode]
        if (editingKey != null) {
            sink.pressKey(editingKey, shift = event.isShiftPressed, ctrl = event.isCtrlPressed)
            return true
        }
        if (event.isCtrlPressed || event.isMetaPressed) {
            // Ctrl+letter is a shortcut, never text. Ctrl+A selects the whole text box.
            if (event.keyCode != KeyEvent.KEYCODE_A || !event.isCtrlPressed) {
                return false
            }
            sink.pressCharacter('a'.code, shift = event.isShiftPressed, ctrl = true)
            return true
        }
        val codePoint = event.unicodeChar
        if (codePoint == 0 || Character.isISOControl(codePoint)) {
            return false
        }
        sink.insertText(String(Character.toChars(codePoint)))
        return true
    }
}
