package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** A double tap on a card opens the notebook once. */
class LaunchGuardTest {
    @Test
    fun aSecondTapBeforeTheEditorShowsIsIgnored() {
        val guard = LaunchGuard()

        assertTrue(guard.tryStart())
        assertFalse(guard.tryStart())
    }

    @Test
    fun backInTheLibraryANotebookOpensAgain() {
        val guard = LaunchGuard()
        guard.tryStart()

        guard.resumed()

        assertTrue(guard.tryStart())
    }
}
