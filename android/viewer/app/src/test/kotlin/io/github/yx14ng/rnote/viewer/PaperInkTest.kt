package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** Black ink on black paper is white ink, and white ink on white paper is black. */
class PaperInkTest {
    private val black = 0xFF00_0000.toInt()
    private val white = 0xFFFF_FFFF.toInt()
    private val darkGray = 0xFF2E_2F33.toInt()
    private val red = 0xFFE0_1B24.toInt()

    @Test
    fun darkInkOnDarkPaperTurnsLight() {
        assertEquals(white, BrushPalette.onPaper(black, paper = black))
        assertEquals(white, BrushPalette.onPaper(darkGray, paper = black))
    }

    @Test
    fun lightInkOnLightPaperTurnsDark() {
        assertEquals(black, BrushPalette.onPaper(white, paper = white))
    }

    @Test
    fun colorsAndInkThatShowStayAsTheyAre() {
        assertEquals(red, BrushPalette.onPaper(red, paper = black))
        assertEquals(black, BrushPalette.onPaper(black, paper = white))
        assertEquals(white, BrushPalette.onPaper(white, paper = black))
    }

    @Test
    fun transparencyIsKept() {
        val halfBlack = 0x8000_0000.toInt()

        assertEquals(0x80FF_FFFF.toInt(), BrushPalette.onPaper(halfBlack, paper = black))
    }
}
