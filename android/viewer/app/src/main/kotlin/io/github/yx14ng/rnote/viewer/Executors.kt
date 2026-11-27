package io.github.yx14ng.rnote.viewer

import java.util.concurrent.Executor
import java.util.concurrent.RejectedExecutionException

/**
 * Runs [task] on this executor, or [orElse] right here when the executor stopped taking work
 * (it was shut down, e.g. as the editor closed), instead of throwing: a thread that hands
 * work on to another must not bring the app down when that one is gone.
 */
internal fun Executor.executeOrElse(task: Runnable, orElse: () -> Unit) {
    try {
        execute(task)
    } catch (_: RejectedExecutionException) {
        orElse()
    }
}
