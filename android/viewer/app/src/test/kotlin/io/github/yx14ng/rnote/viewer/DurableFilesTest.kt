package io.github.yx14ng.rnote.viewer

import java.io.File
import java.io.IOException
import java.nio.file.Files
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

/** A crash or a full disk never leaves a half-written file, nor a leftover partial one. */
class DurableFilesTest {
    private val dir: File = Files.createTempDirectory("durable").toFile()

    @Test
    fun replacingLeavesTheNewContentAndNothingElse() {
        val target = File(dir, "a.rnote").apply { writeBytes(byteArrayOf(1)) }

        DurableFiles.replace(target, byteArrayOf(2, 3), ".a.rnote.saving")

        assertArrayEquals(byteArrayOf(2, 3), target.readBytes())
        assertEquals(listOf("a.rnote"), dir.list()!!.toList())
    }

    @Test
    fun aFailedReplaceThrowsAndLeavesNoPartialFile() {
        // A folder with something in it can't be replaced by a file.
        val target = File(dir, "a.rnote").apply { mkdir() }
        File(target, "inside").writeBytes(byteArrayOf(1))

        assertThrows(IOException::class.java) { DurableFiles.replace(target, byteArrayOf(2), ".a.rnote.saving") }

        assertEquals(listOf("a.rnote"), dir.list()!!.toList())
    }
}
