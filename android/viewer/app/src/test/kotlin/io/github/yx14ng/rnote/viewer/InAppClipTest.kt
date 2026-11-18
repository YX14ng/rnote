package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Copies too big for the system clipboard stay in the app, behind a marker. */
class InAppClipTest {
    private val big = ClipContent(ClipContent.STROKE_CONTENT_MIME_TYPE, "x".repeat(InAppClip.SYSTEM_LIMIT_CHARS + 1))
    private val small = ClipContent(ClipContent.STROKE_CONTENT_MIME_TYPE, "{\"strokes\":[]}")

    @Test
    fun onlyBigCopiesStayInTheApp() {
        assertTrue(InAppClip.tooBig(big))
        assertFalse(InAppClip.tooBig(small))
    }

    @Test
    fun theMarkerItLeftGivesTheCopyBack() {
        val clip = InAppClip()
        val marker = clip.keep(big)

        assertEquals(big, clip.lookup(isMarker = true, text = marker))
    }

    @Test
    fun anotherCopyOnTheSystemClipboardWins() {
        val clip = InAppClip()
        val marker = clip.keep(big)

        assertNull(clip.lookup(isMarker = false, text = marker))
        assertNull(clip.lookup(isMarker = true, text = "an older marker"))
    }
}
