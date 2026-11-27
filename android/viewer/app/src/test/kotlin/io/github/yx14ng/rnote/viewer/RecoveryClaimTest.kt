package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Whose the recovery copy is: only that document's saving may delete or replace it. */
class RecoveryClaimTest {
    @Test
    fun aCopyKeptFromBeforeBelongsToNoOpenDocument() {
        val claim = RecoveryClaim()

        assertFalse(claim.mayClear())
        assertFalse(claim.mayWrite(copyExists = true))
    }

    @Test
    fun theDocumentThatWroteOrRestoredTheCopyOwnsIt() {
        val wrote = RecoveryClaim().apply { wrote() }
        val restored = RecoveryClaim().apply { restored() }

        assertTrue(wrote.mayClear())
        assertTrue(restored.mayClear())
        assertTrue(restored.mayWrite(copyExists = true))
    }

    @Test
    fun openingAnotherDocumentLeavesTheCopyToItsOwner() {
        val claim = RecoveryClaim().apply { restored() }

        claim.opened()

        assertFalse(claim.mayClear())
        assertFalse(claim.mayWrite(copyExists = true))
    }

    @Test
    fun withoutACopyAnyDocumentMayWriteOne() {
        assertTrue(RecoveryClaim().mayWrite(copyExists = false))
    }
}
