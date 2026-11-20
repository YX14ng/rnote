package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Whether the selector's touch draws a new lasso or moves the selection, guessed at once. */
class GesturePreviewTest {
    private val selection = DocumentBounds(100.0, 100.0, 300.0, 200.0)

    @Test
    fun withoutASelectionEveryTouchDrawsALasso() {
        assertTrue(GesturePreview.startsLasso(150.0, 150.0, selection = null, margin = 10.0))
    }

    @Test
    fun aTouchOnTheSelectionOrItsHandlesMovesIt() {
        assertFalse(GesturePreview.startsLasso(150.0, 150.0, selection, margin = 10.0))
        // A corner handle sticks out of the bounds a little.
        assertFalse(GesturePreview.startsLasso(305.0, 95.0, selection, margin = 10.0))
    }

    @Test
    fun aTouchAwayFromTheSelectionDrawsANewLasso() {
        assertTrue(GesturePreview.startsLasso(500.0, 150.0, selection, margin = 10.0))
    }
}
