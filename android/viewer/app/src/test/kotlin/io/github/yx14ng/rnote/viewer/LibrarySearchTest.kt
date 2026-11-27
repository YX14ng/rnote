package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Finding notebooks by name, what an empty screen says, and keeping the section on rotation. */
class LibrarySearchTest {
    @Test
    fun aNameMatchesEveryWordInAnyOrderCaseOrAccent() {
        assertTrue(LibrarySearch.matches("Álgebra Lineal - Parcial 2", "parcial algebra"))
        assertTrue(LibrarySearch.matches("Física II", "FISICA"))
        assertFalse(LibrarySearch.matches("Álgebra Lineal", "algebra orga"))
    }

    @Test
    fun anEmptySearchMatchesEverything() {
        assertTrue(LibrarySearch.matches("Algo", "   "))
    }

    @Test
    fun searchingLooksThroughEveryNotebookButTheTrash() {
        val notebooks = listOf(
            Notebook("Orga/Parcial.rnote", "Parcial", "Orga", false, 0, 0, emptyList(), key = "a"),
            Notebook("Otro.rnote", "Otro", null, false, 0, 0, emptyList(), key = "b"),
        )
        val snapshot = LibrarySnapshot(notebooks, listOf("Orga"), emptyList(), emptyList())

        val found = LibrarySearch.find(snapshot, "parcial")

        assertEquals(listOf("Orga/Parcial.rnote"), found.map { it.path })
    }

    @Test
    fun anEmptyScreenSaysWhy() {
        assertEquals(R.string.library_empty_search, LibrarySearch.emptyMessage(LibrarySection.Folder("Orga"), "xyz"))
        assertEquals(R.string.library_trash_is_empty, LibrarySearch.emptyMessage(LibrarySection.Trash, ""))
        assertEquals(R.string.library_empty_folder, LibrarySearch.emptyMessage(LibrarySection.Folder("Orga"), ""))
        assertEquals(R.string.library_empty_tag, LibrarySearch.emptyMessage(LibrarySection.Tag("t1"), ""))
        assertNull(LibrarySearch.emptyMessage(LibrarySection.All, ""))
    }

    @Test
    fun aSectionSurvivesAsText() {
        val sections = listOf(
            LibrarySection.All,
            LibrarySection.Uncategorized,
            LibrarySection.Folder("Orga: 2"),
            LibrarySection.Tag("t1"),
            LibrarySection.Trash,
        )
        for (section in sections) {
            assertEquals(section, LibrarySearch.sectionFrom(LibrarySearch.sectionText(section)))
        }
        assertEquals(LibrarySection.All, LibrarySearch.sectionFrom(null))
    }
}
