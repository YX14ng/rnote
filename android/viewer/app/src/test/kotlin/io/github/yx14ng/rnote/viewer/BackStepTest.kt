package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** Back closes what is open on top of the page before it leaves the notebook. */
class BackStepTest {
    @Test
    fun theTopmostOpenThingClosesFirst() {
        assertEquals(BackStep.OVERLAY, BackStep.first(overlay = true, search = true, pages = true, selection = true))
        assertEquals(BackStep.SEARCH, BackStep.first(overlay = false, search = true, pages = true, selection = true))
        assertEquals(BackStep.PAGES, BackStep.first(overlay = false, search = false, pages = true, selection = true))
        assertEquals(BackStep.SELECTION, BackStep.first(overlay = false, search = false, pages = false, selection = true))
    }

    @Test
    fun withNothingOpenBackLeaves() {
        assertEquals(BackStep.LEAVE, BackStep.first(overlay = false, search = false, pages = false, selection = false))
    }
}
