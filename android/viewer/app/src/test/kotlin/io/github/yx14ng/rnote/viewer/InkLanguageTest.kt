package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** The languages the handwriting is read in, as ML Kit names its models. */
class InkLanguageTest {
    @Test
    fun tagsAreTheModelTags() {
        assertEquals("es", InkLanguage.ES.tag)
        assertEquals("en-US", InkLanguage.EN.tag)
        assertEquals("zh-Hani-CN", InkLanguage.ZH.tag)
        assertEquals("zh-Hani-TW", InkLanguage.ZH_HANT.tag)
        assertEquals("ja", InkLanguage.JA.tag)
        assertEquals("ko", InkLanguage.KO.tag)
        assertEquals("ru", InkLanguage.RU.tag)
    }

    @Test
    fun chineseAndJapaneseAreWrittenWithoutSpaces() {
        val unspaced = InkLanguage.entries.filterNot { it.spaced }.toSet()

        assertEquals(setOf(InkLanguage.ZH, InkLanguage.ZH_HANT, InkLanguage.JA), unspaced)
    }

    @Test
    fun aKnownTagGivesItsLanguage() {
        for (language in InkLanguage.entries) {
            assertEquals(language, InkLanguage.fromTag(language.tag))
        }
    }

    @Test
    fun anUnknownTagFallsBackToSpanish() {
        assertEquals(InkLanguage.ES, InkLanguage.fromTag("fr"))
        assertEquals(InkLanguage.ES, InkLanguage.fromTag(null))
    }
}
