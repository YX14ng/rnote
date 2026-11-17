package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** What goes before a word for the recognizer: the end of the word before it and a space. */
class InkPreContextTest {
    @Test
    fun theWordBeforeAndASpace() {
        assertEquals("la ", InkIndexer.preContext("la"))
    }

    @Test
    fun nothingBeforeTheFirstWordOfALine() {
        assertEquals("", InkIndexer.preContext(null))
        assertEquals("", InkIndexer.preContext(" "))
    }

    @Test
    fun atMostTwentyCharacters() {
        val context = InkIndexer.preContext("transformaciones lineales")

        assertEquals("ormaciones lineales ", context)
    }
}
