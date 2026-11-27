package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The circle under the hovering S Pen shows what touching would draw or erase. */
class HoverCursorTest {
    @Test
    fun thePenShowsItsWidthAtTheZoom() {
        assertEquals(3f, HoverCursor.radiusPx(Tool.BRUSH, eraserEnd = false, brushWidth = 2.0, zoom = 3.0)!!, 0.001f)
    }

    @Test
    fun theEraserShowsItsWidth() {
        val expected = (HoverCursor.ERASER_WIDTH * 2.0 / 2).toFloat()
        assertEquals(expected, HoverCursor.radiusPx(Tool.ERASER, eraserEnd = false, brushWidth = 2.0, zoom = 2.0)!!, 0.001f)
        assertEquals(expected, HoverCursor.radiusPx(Tool.BRUSH, eraserEnd = true, brushWidth = 2.0, zoom = 2.0)!!, 0.001f)
    }

    @Test
    fun aThinPenStillShowsADot() {
        assertEquals(HoverCursor.MIN_RADIUS_PX, HoverCursor.radiusPx(Tool.BRUSH, eraserEnd = false, brushWidth = 0.5, zoom = 0.5)!!, 0.001f)
    }

    @Test
    fun otherToolsShowNothing() {
        assertNull(HoverCursor.radiusPx(Tool.SELECTOR, eraserEnd = false, brushWidth = 2.0, zoom = 1.0))
    }
}
