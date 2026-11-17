package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** What goes before a word for the recognizer: the end of the word before it and a space. */
class InkPreContextTest {
    @Test
    fun theWordBeforeAndASpace() {
        assertEquals("la ", InkIndexer.preContext("la", InkLanguage.ES))
    }

    @Test
    fun nothingBeforeTheFirstWordOfALine() {
        assertEquals("", InkIndexer.preContext(null, InkLanguage.ES))
        assertEquals("", InkIndexer.preContext(" ", InkLanguage.ES))
    }

    @Test
    fun atMostTwentyCharacters() {
        val context = InkIndexer.preContext("transformaciones lineales", InkLanguage.ES)

        assertEquals("ormaciones lineales ", context)
    }

    @Test
    fun noSpaceAfterChineseOrJapanese() {
        assertEquals("线性", InkIndexer.preContext("线性", InkLanguage.ZH))
        assertEquals("行列", InkIndexer.preContext("行列", InkLanguage.JA))
        assertEquals("선형 ", InkIndexer.preContext("선형", InkLanguage.KO))
    }

    @Test
    fun atMostTwentyCharactersWithoutSpaces() {
        assertEquals(20, InkIndexer.preContext("一二三四五六七八九十一二三四五六七八九十一二", InkLanguage.ZH).length)
    }
}
