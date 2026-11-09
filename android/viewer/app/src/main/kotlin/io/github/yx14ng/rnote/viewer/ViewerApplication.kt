package io.github.yx14ng.rnote.viewer

import android.app.Application
import android.content.Context
import android.system.Os
import java.io.File

/** Prepares the native environment before any document is opened. */
class ViewerApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        configureFontconfig(this)
    }

    private companion object {
        /**
         * The bundled fontconfig looks for its config inside the build prefix, which doesn't
         * exist on the device. Point it at the system fonts and a cache it can write to.
         *
         * Must run before the first text is rendered, since fontconfig reads it only once.
         */
        fun configureFontconfig(context: Context) {
            val cacheDir = File(context.cacheDir, "fontconfig").apply { mkdirs() }
            val config = File(context.filesDir, "fonts.conf")
            config.writeText(
                """
                |<?xml version="1.0"?>
                |<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
                |<fontconfig>
                |  <dir>/system/fonts</dir>
                |  <cachedir>${cacheDir.absolutePath}</cachedir>
                |</fontconfig>
                |
                """.trimMargin(),
            )
            Os.setenv("FONTCONFIG_FILE", config.absolutePath, true)
        }
    }
}
