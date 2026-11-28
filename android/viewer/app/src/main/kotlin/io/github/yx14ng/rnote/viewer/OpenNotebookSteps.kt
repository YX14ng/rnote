package io.github.yx14ng.rnote.viewer

/** The decisions of [OpenNotebookSync], apart from files and threads. */
internal object OpenNotebookSteps {
    sealed interface Step {
        /** The sync left the open notebook alone. */
        data object Nothing : Step

        /** The notebook moved to [to] here: the editor goes on with it there. */
        data class Follow(val to: String) : Step

        /** The sync wrote the notebook's file: see whether it is still what the editor has. */
        data object Check : Step
    }

    enum class Outcome { Keep, Reopen, Close }

    /** What to do with the notebook open at [path] after a sync that did [report]. */
    fun afterSync(path: String, report: SyncReport): Step {
        report.movedTo(path)?.let { return Step.Follow(it) }
        return if (path in report.changedHere) Step.Check else Step.Nothing
    }

    /**
     * What to do once the file holds [current] (its hash; null when gone) and the editor read
     * or wrote [known]: [stillOpen] is false when the editor moved on meanwhile. With unsaved
     * changes ([modified]) nothing happens: saving makes a conflict copy.
     */
    fun afterCheck(stillOpen: Boolean, modified: Boolean, known: String?, current: String?): Outcome = when {
        !stillOpen || modified || current == known -> Outcome.Keep
        current != null -> Outcome.Reopen
        else -> Outcome.Close
    }
}
