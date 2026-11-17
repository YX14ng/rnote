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
