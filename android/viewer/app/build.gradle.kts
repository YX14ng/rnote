plugins {
    id("com.android.application")
}

android {
    namespace = "io.github.yx14ng.rnote.viewer"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.yx14ng.rnote.viewer"
        // API 28 is the first with iconv in bionic, which glib needs.
        minSdk = 28
        targetSdk = 36
        versionCode = 2
        versionName = "0.1.1"

        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    // The stable key every published APK is signed with, so the OAuth client registered
    // with its SHA-1 keeps working and installs can be updated. See android/README.md.
    val stableKeystore = System.getenv("RNOTE_KEYSTORE")
    signingConfigs {
        if (stableKeystore != null) {
            create("stable") {
                storeFile = file(stableKeystore)
                storePassword = System.getenv("RNOTE_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("RNOTE_KEY_ALIAS") ?: "rnote"
                keyPassword = System.getenv("RNOTE_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        debug {
            if (stableKeystore != null) {
                signingConfig = signingConfigs.getByName("stable")
            }
        }
        release {
            if (stableKeystore != null) {
                signingConfig = signingConfigs.getByName("stable")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

// A release must be signed with the stable key: an APK signed otherwise couldn't update
// installed apps nor sign in to Google Drive.
gradle.taskGraph.whenReady {
    if (System.getenv("RNOTE_KEYSTORE") == null && allTasks.any { it.name.contains("Release") }) {
        throw GradleException("RNOTE_KEYSTORE is not set: see android/README.md, \"Signing\"")
    }
}
