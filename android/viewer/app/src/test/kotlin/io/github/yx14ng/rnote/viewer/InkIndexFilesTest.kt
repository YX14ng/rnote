package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** The handwriting indexes of notebooks that are gone don't pile up. */
class InkIndexFilesTest {
    private val day = 24L * 60 * 60 * 1000

    @Test
    fun indexesOfNotebooksGoneAndUnusedForAMonthAreStale() {
        val now = 100 * day
        val files = listOf(
            "live.json" to now - 90 * day,
            "gone-old.json" to now - 40 * day,
            "gone-recent.json" to now - 2 * day,
        )

        assertEquals(listOf("gone-old.json"), InkIndexFiles.stale(files, live = setOf("live.json"), now = now))
    }

    @Test
    fun aMovedNotebookTakesItsIndexAlong() {
        assertEquals(
            InkIndexFiles.nameFor("Orga/b.rnote"),
            InkIndexFiles.nameFor("Orga/b.rnote"),
        )
        assert(InkIndexFiles.nameFor("a.rnote") != InkIndexFiles.nameFor("b.rnote"))
    }
}
