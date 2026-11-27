package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.FileNotFoundException
import java.io.IOException
import java.net.SocketTimeoutException
import java.net.UnknownHostException

/** What went wrong, as the user is told it, from the errors the app actually gets. */
class ErrorKindTest {
    @Test
    fun aFileTheEngineCannotReadIsDamaged() {
        val error = RuntimeException(
            "Rust error: Loading the .rnote file failed: loading RnoteFile from bytes failed.: " +
                "decompressing bytes failed.: unexpected end of file",
        )

        assertEquals(ErrorKind.DAMAGED, ErrorKind.of(error))
    }

    @Test
    fun aFullDiskSaysSo() {
        assertEquals(ErrorKind.NO_SPACE, ErrorKind.of(IOException("write failed: ENOSPC (No space left on device)")))
    }

    @Test
    fun aMissingFileSaysSo() {
        assertEquals(ErrorKind.NOT_FOUND, ErrorKind.of(FileNotFoundException("/sdcard/a.rnote: open failed: ENOENT")))
    }

    @Test
    fun aRefusedAccessSaysSo() {
        assertEquals(ErrorKind.NO_ACCESS, ErrorKind.of(SecurityException("Permission Denial: writing content://x")))
        assertEquals(ErrorKind.NO_ACCESS, ErrorKind.of(IOException("open failed: EACCES (Permission denied)")))
    }

    @Test
    fun noNetworkSaysOffline() {
        assertEquals(ErrorKind.OFFLINE, ErrorKind.of(UnknownHostException("www.googleapis.com")))
        assertEquals(ErrorKind.OFFLINE, ErrorKind.of(IOException("wrapped", SocketTimeoutException("timeout"))))
    }

    @Test
    fun runningOutOfMemoryIsTooBig() {
        assertEquals(ErrorKind.TOO_BIG, ErrorKind.of(OutOfMemoryError("Failed to allocate")))
    }

    @Test
    fun anythingElseIsUnexpected() {
        assertEquals(ErrorKind.UNEXPECTED, ErrorKind.of(IllegalStateException("something odd")))
    }
}
