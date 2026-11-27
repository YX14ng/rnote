package io.github.yx14ng.rnote.viewer

import java.io.File
import java.io.FileOutputStream
import java.io.IOException

/** Replacing the app's own files so that a crash, a power cut or a full disk can't break them. */
internal object DurableFiles {
    /**
     * Replaces [target] with [bytes] through [temporaryName], a file next to it: the bytes
     * reach the storage before the rename, so a crash leaves the old file or the new one.
     * When anything fails the partial file is removed, so a full disk doesn't keep its space.
     */
    fun replace(target: File, bytes: ByteArray, temporaryName: String) {
        val temporary = File(target.parentFile, temporaryName)
        try {
            FileOutputStream(temporary).use { output ->
                output.write(bytes)
                output.fd.sync()
            }
            if (!temporary.renameTo(target)) throw IOException("Cannot replace $target")
        } catch (error: IOException) {
            temporary.delete()
            throw error
        }
    }
}
