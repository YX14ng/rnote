package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** Each brush style keeps its own color and width. */
class BrushMemoryTest {
    private val red = Brush(0xFFD03030.toInt(), 1.3)
    private val blue = Brush(0xFF3050D0.toInt(), 2.0)

    @Test
    fun switchingStylesBringsBackWhatEachOneHad() {
        val memory = mapOf(BrushStyle.SOLID to blue)

        val (kept, brush) = BrushMemory.switch(memory, from = BrushStyle.BALLPOINT, current = red, to = BrushStyle.SOLID)

        assertEquals(blue, brush)
        assertEquals(red, kept[BrushStyle.BALLPOINT])
    }

    @Test
    fun theMarkerStartsAsAHighlighter() {
        val (_, brush) = BrushMemory.switch(emptyMap(), from = BrushStyle.BALLPOINT, current = red, to = BrushStyle.MARKER)

        assertEquals(BrushMemory.HIGHLIGHTER, brush)
    }

    @Test
    fun aPenNeverStartsSeeThrough() {
        val highlight = Brush(0x66FFD600, 12.0)

        val (_, brush) = BrushMemory.switch(emptyMap(), from = BrushStyle.MARKER, current = highlight, to = BrushStyle.SOLID)

        assertEquals(0xFFFFD600.toInt(), brush.color)
        assertEquals(BrushPalette.DEFAULT_WIDTH, brush.width, 0.0)
    }

    @Test
    fun aStyleNeverSeenTakesThePenAsItIs() {
        val (_, brush) = BrushMemory.switch(emptyMap(), from = BrushStyle.BALLPOINT, current = red, to = BrushStyle.TEXTURED)

        assertEquals(red, brush)
    }
}
