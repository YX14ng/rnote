package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** What the editor does with its notebook after a sync touched the library. */
class OpenNotebookStepsTest {
    private fun report(changedHere: List<String> = emptyList(), movedHere: Map<String, String> = emptyMap()) =
        SyncReport(0, 0, movedHere.size, 0, emptyList(), changedHere, emptyList(), movedHere)

    @Test
    fun aMoveIsFollowedEvenIfTheOldPathAlsoChanged() {
        val report = report(changedHere = listOf("a.rnote"), movedHere = mapOf("a.rnote" to "Math/a.rnote"))
        assertEquals(OpenNotebookSteps.Step.Follow("Math/a.rnote"), OpenNotebookSteps.afterSync("a.rnote", report))
    }

    @Test
    fun aChangeHereIsChecked_andOtherNotebooksAreIgnored() {
        assertEquals(OpenNotebookSteps.Step.Check, OpenNotebookSteps.afterSync("a.rnote", report(changedHere = listOf("a.rnote"))))
        assertEquals(OpenNotebookSteps.Step.Nothing, OpenNotebookSteps.afterSync("a.rnote", report(changedHere = listOf("b.rnote"))))
    }

    @Test
    fun aNewerFileReopensUnlessThereAreUnsavedChanges() {
        assertEquals(OpenNotebookSteps.Outcome.Reopen, OpenNotebookSteps.afterCheck(stillOpen = true, modified = false, known = "x", current = "y"))
        assertEquals(OpenNotebookSteps.Outcome.Keep, OpenNotebookSteps.afterCheck(stillOpen = true, modified = true, known = "x", current = "y"))
    }

    @Test
    fun aDeletedFileClosesOnlyIfClean_andTheSameFileOrAnotherNotebookDoesNothing() {
        assertEquals(OpenNotebookSteps.Outcome.Close, OpenNotebookSteps.afterCheck(stillOpen = true, modified = false, known = "x", current = null))
        assertEquals(OpenNotebookSteps.Outcome.Keep, OpenNotebookSteps.afterCheck(stillOpen = true, modified = true, known = "x", current = null))
        assertEquals(OpenNotebookSteps.Outcome.Keep, OpenNotebookSteps.afterCheck(stillOpen = true, modified = false, known = "x", current = "x"))
        assertEquals(OpenNotebookSteps.Outcome.Keep, OpenNotebookSteps.afterCheck(stillOpen = false, modified = false, known = "x", current = "y"))
    }
}
