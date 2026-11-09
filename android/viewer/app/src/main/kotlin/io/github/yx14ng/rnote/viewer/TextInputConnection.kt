package io.github.yx14ng.rnote.viewer

import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import java.text.BreakIterator

/** Where typed input goes: the text tool, through the engine. */
internal interface TextSink {
    fun insertText(text: String)

    fun pressKey(key: EditingKey, shift: Boolean = false, ctrl: Boolean = false)
}

/**
 * Connects the soft keyboard to the text tool.
 *
 * The engine owns the text, so this keeps no editable buffer of its own. Composing text (the
 * word the keyboard is still suggesting for) is typed into the document right away and
 * replaced with backspaces as it changes, so it shows up live.
 */
internal class TextInputConnection(view: View, private val sink: TextSink) :
    BaseInputConnection(view, /* fullEditor */ false) {
    private var composing = ""

    override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
        replaceComposing(text.toString())
        return true
    }

    override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
        replaceComposing(text.toString())
        composing = ""
        return true
    }

    override fun finishComposingText(): Boolean {
        composing = ""
        return true
    }

    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
        composing = ""
        repeat(beforeLength) { sink.pressKey(EditingKey.BACKSPACE) }
        repeat(afterLength) { sink.pressKey(EditingKey.DELETE) }
        return true
    }

    override fun sendKeyEvent(event: KeyEvent): Boolean {
        if (event.action == KeyEvent.ACTION_DOWN) {
            composing = ""
            KeyMapping.dispatch(event, sink)
        }
        return true
    }

    /** Turns the composing text into [text], touching only what differs at the end. */
    private fun replaceComposing(text: String) {
        val kept = composing.commonPrefixWith(text).length
        repeat(graphemeCount(composing.substring(kept))) { sink.pressKey(EditingKey.BACKSPACE) }
        val added = text.substring(kept)
        if (added.isNotEmpty()) {
            sink.insertText(added)
        }
        composing = text
    }

    /** Backspace removes one grapheme (user-perceived character) in the engine. */
    private fun graphemeCount(text: String): Int {
        if (text.isEmpty()) {
            return 0
        }
        val iterator = BreakIterator.getCharacterInstance()
        iterator.setText(text)
        var count = 0
        while (iterator.next() != BreakIterator.DONE) {
            count++
        }
        return count
    }
}
