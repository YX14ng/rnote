package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** A new thumbnail of a notebook replaces the ones of its earlier versions. */
class ThumbnailFilesTest {
    @Test
    fun theOlderVersionsOfTheSameNotebookAreLeftOver() {
        val names = listOf(
            "aaaaaaaaaaaaaaaa-0000000000000001.png",
            "aaaaaaaaaaaaaaaa-0000000000000002.png",
            "bbbbbbbbbbbbbbbb-0000000000000001.png",
            "aaaaaaaaaaaaaaaa-0000000000000003.tmp",
        )

        assertEquals(
            listOf("aaaaaaaaaaaaaaaa-0000000000000001.png"),
            ThumbnailFiles.olderVersions(names, current = "aaaaaaaaaaaaaaaa-0000000000000002"),
        )
    }
}
