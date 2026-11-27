package io.github.yx14ng.rnote.viewer

import java.util.concurrent.Executors
import org.junit.Assert.assertTrue
import org.junit.Test

/** Work handed to a thread that already stopped, e.g. a save finishing as the editor closes. */
class ExecutorsTest {
    @Test
    fun aStoppedExecutorRunsTheFallbackInsteadOfThrowing() {
        val executor = Executors.newSingleThreadExecutor()
        executor.shutdown()
        var fellBack = false

        executor.executeOrElse({ error("must not run") }) { fellBack = true }

        assertTrue(fellBack)
    }
}
