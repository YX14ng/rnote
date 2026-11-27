package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** Conflict copies are told by name, as the library shows them. */
class ConflictCopiesTest {
    @Test
    fun aCopyIsNamedWithoutFolderOrExtension() {
        assertEquals(
            "Parcial (conflicto, PC, 2026-11-12 17.20)",
            ConflictCopies.name("Orga/Parcial (conflicto, PC, 2026-11-12 17.20).rnote"),
        )
        assertEquals("Suelto", ConflictCopies.name("Suelto.rnote"))
    }
}
