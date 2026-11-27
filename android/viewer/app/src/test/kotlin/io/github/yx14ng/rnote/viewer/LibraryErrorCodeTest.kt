package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The library's errors come from the engine as codes, worded here in the user's language. */
class LibraryErrorCodeTest {
    @Test
    fun aCodeKeepsItsArgumentWhole() {
        val error = RuntimeException("Rust error: library-error:name_taken:Orga: 2")

        assertEquals(LibraryErrorCode("name_taken", "Orga: 2"), LibraryErrorCode.of(error))
    }

    @Test
    fun aCodeMayHaveNoArgument() {
        assertEquals(LibraryErrorCode("empty_name", ""), LibraryErrorCode.of(RuntimeException("library-error:empty_name:")))
    }

    @Test
    fun otherErrorsHaveNoCode() {
        assertNull(LibraryErrorCode.of(RuntimeException("disk exploded")))
    }
}
