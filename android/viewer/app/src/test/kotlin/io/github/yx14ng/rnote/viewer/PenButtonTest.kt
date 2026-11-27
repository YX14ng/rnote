package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** What the stylus does with its button held, as the user set it. */
class PenButtonTest {
    @Test
    fun theButtonDoesWhatWasChosen() {
        assertEquals(Tool.ERASER, PenButton.ERASER.toolFor(eraserEnd = false, buttonHeld = true, selected = Tool.BRUSH))
        assertEquals(Tool.SELECTOR, PenButton.LASSO.toolFor(eraserEnd = false, buttonHeld = true, selected = Tool.BRUSH))
        assertEquals(Tool.BRUSH, PenButton.NOTHING.toolFor(eraserEnd = false, buttonHeld = true, selected = Tool.BRUSH))
    }

    @Test
    fun withoutTheButtonThePenDrawsWithTheSelectedTool() {
        assertEquals(Tool.TYPEWRITER, PenButton.LASSO.toolFor(eraserEnd = false, buttonHeld = false, selected = Tool.TYPEWRITER))
    }

    @Test
    fun theEraserEndAlwaysErases() {
        assertEquals(Tool.ERASER, PenButton.NOTHING.toolFor(eraserEnd = true, buttonHeld = false, selected = Tool.BRUSH))
    }

    @Test
    fun tappingTheSettingGoesThroughTheChoices() {
        assertEquals(PenButton.LASSO, PenButton.ERASER.next())
        assertEquals(PenButton.NOTHING, PenButton.LASSO.next())
        assertEquals(PenButton.ERASER, PenButton.NOTHING.next())
    }
}
