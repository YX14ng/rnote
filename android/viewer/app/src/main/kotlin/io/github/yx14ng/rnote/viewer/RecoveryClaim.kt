package io.github.yx14ng.rnote.viewer

/**
 * Whether the recovery copy belongs to the document open in the editor: it wrote the copy
 * or was restored from it. Only then may saving that document delete or replace the copy;
 * a copy kept from before (e.g. "Keep" after a failed restore) waits for its own document.
 */
internal class RecoveryClaim {
    private var owned = false

    /** The open document wrote the copy. */
    fun wrote() {
        owned = true
    }

    /** The open document came from the copy. */
    fun restored() {
        owned = true
    }

    /** Another document was opened: the copy, if any, isn't its. */
    fun opened() {
        owned = false
    }

    /** Whether saving the open document may delete the copy. */
    fun mayClear(): Boolean = owned

    /** Whether the open document may write its unsaved changes as the copy. */
    fun mayWrite(copyExists: Boolean): Boolean = owned || !copyExists
}
