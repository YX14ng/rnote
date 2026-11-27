package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** Holding a notebook's card selects it, as in other apps' libraries. */
class CardSelectionTest {
    @Test
    fun holdingACardStartsSelectingWithIt() {
        assertEquals(setOf("a.rnote"), CardSelection.held(selection = null, path = "a.rnote"))
    }

    @Test
    fun whileSelectingHoldingACardTogglesIt() {
        assertEquals(setOf("a.rnote", "b.rnote"), CardSelection.held(setOf("a.rnote"), "b.rnote"))
        assertEquals(emptySet<String>(), CardSelection.held(setOf("a.rnote"), "a.rnote"))
    }
}
