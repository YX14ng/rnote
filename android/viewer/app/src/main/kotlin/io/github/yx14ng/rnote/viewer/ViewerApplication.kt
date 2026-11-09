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
         * exist on the device. Point it at the system fonts and a cache it can write to, and map
         * the generic families documents use (the desktop app defaults to `serif`) to fonts
         * Android ships.
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
                |  <alias><family>serif</family><prefer><family>Noto Serif</family></prefer></alias>
                |  <alias><family>sans-serif</family><prefer><family>Roboto</family></prefer></alias>
                |  <alias><family>sans</family><prefer><family>Roboto</family></prefer></alias>
                |  <alias><family>monospace</family><prefer><family>Droid Sans Mono</family><family>Cutive Mono</family></prefer></alias>
                |  <alias><family>cursive</family><prefer><family>Dancing Script</family></prefer></alias>
                |  <!-- Synthetic bold and italic for fonts without those faces, as in fontconfig's
                |       90-synthetic.conf, which this minimal config doesn't include. -->
                |  <match target="font">
                |    <test name="weight" compare="less_eq"><const>medium</const></test>
                |    <test target="pattern" name="weight" compare="more_eq"><const>bold</const></test>
                |    <edit name="embolden" mode="assign"><bool>true</bool></edit>
                |    <edit name="weight" mode="assign"><const>bold</const></edit>
                |  </match>
                |  <match target="font">
                |    <test name="slant"><const>roman</const></test>
                |    <test target="pattern" name="slant" compare="not_eq"><const>roman</const></test>
                |    <edit name="matrix" mode="assign">
                |      <times><name>matrix</name><matrix><double>1</double><double>0.2</double><double>0</double><double>1</double></matrix></times>
                |    </edit>
                |    <edit name="slant" mode="assign"><const>oblique</const></edit>
                |  </match>
                |</fontconfig>
                |
                """.trimMargin(),
            )
            Os.setenv("FONTCONFIG_FILE", config.absolutePath, true)
        }
    }
}
