package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** The live preview of a stroke must be as wide as the stroke the engine leaves on the page. */
class StrokeWidthTest {
    @Test
    fun solidAndTexturedFollowThePressureLinearly() {
        for (style in listOf(BrushStyle.SOLID, BrushStyle.TEXTURED)) {
            assertEquals(1.5, StrokeWidth.at(style, width = 3.0, pressure = 0.5), 1e-9)
            assertEquals(0.75, StrokeWidth.at(style, width = 3.0, pressure = 0.25), 1e-9)
            assertEquals(3.0, StrokeWidth.at(style, width = 3.0, pressure = 1.0), 1e-9)
        }
    }

    @Test
    fun markerIgnoresThePressure() {
        assertEquals(3.0, StrokeWidth.at(BrushStyle.MARKER, width = 3.0, pressure = 0.2), 1e-9)
    }
}
