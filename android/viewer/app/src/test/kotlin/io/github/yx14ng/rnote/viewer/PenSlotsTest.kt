package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** The pen and the highlighter are one tap apart, each keeping its own style. */
class PenSlotsTest {
    @Test
    fun theHighlighterIsTheMarkerStyle() {
        assertEquals(BrushStyle.MARKER, PenSlots.styleFor(PenSlots.Slot.HIGHLIGHTER, penStyle = BrushStyle.SOLID))
    }

    @Test
    fun thePenGoesBackToTheStyleItHad() {
        assertEquals(BrushStyle.TEXTURED, PenSlots.styleFor(PenSlots.Slot.PEN, penStyle = BrushStyle.TEXTURED))
    }

    @Test
    fun choosingAPenStyleRemembersItButTheMarkerDoesnt() {
        assertEquals(BrushStyle.BALLPOINT, PenSlots.penStyleAfter(BrushStyle.BALLPOINT, previous = BrushStyle.SOLID))
        assertEquals(BrushStyle.SOLID, PenSlots.penStyleAfter(BrushStyle.MARKER, previous = BrushStyle.SOLID))
    }
}
