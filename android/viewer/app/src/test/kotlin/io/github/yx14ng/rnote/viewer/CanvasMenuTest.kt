package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** What holding a finger on the page offers. */
class CanvasMenuTest {
    @Test
    fun withoutSelectionItPastesThereOrSelectsAll() {
        val actions = CanvasMenu.entries(hasSelection = false, x = 10.0, y = 20.0).map { it.action }

        assertEquals(
            listOf(EditorMenu.Action.PasteAt(10.0, 20.0), EditorMenu.Action.Selection(SelectionAction.SELECT_ALL)),
            actions,
        )
    }

    @Test
    fun withSelectionItAlsoActsOnIt() {
        val actions = CanvasMenu.entries(hasSelection = true, x = 10.0, y = 20.0).map { it.action }

        assertEquals(
            listOf(
                EditorMenu.Action.PasteAt(10.0, 20.0),
                EditorMenu.Action.Copy,
                EditorMenu.Action.Cut,
                EditorMenu.Action.Selection(SelectionAction.DUPLICATE),
                EditorMenu.Action.Selection(SelectionAction.DELETE),
            ),
            actions,
        )
    }

    @Test
    fun onlyDeletingLooksDangerous() {
        val dangerous = CanvasMenu.entries(hasSelection = true, x = 0.0, y = 0.0).filter { it.danger }.map { it.action }

        assertEquals(listOf(EditorMenu.Action.Selection(SelectionAction.DELETE)), dangerous)
    }
}
