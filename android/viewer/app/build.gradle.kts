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
        versionCode = 3
        versionName = "0.2.0"

        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }

        // Google identifies the app by its package and the SHA-1 of the stable key: an APK
        // signed with another key can't sign in, so its sync is off and says why.
        buildConfigField("boolean", "DRIVE_SYNC", (System.getenv("RNOTE_KEYSTORE") != null).toString())
    }

    buildFeatures {
        buildConfig = true
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

dependencies {
    // The Authorization API, for the access token of the Drive sync.
    implementation("com.google.android.gms:play-services-auth:22.0.0")
}

// A release must be signed with the stable key: an APK signed otherwise couldn't update
// installed apps nor sign in to Google Drive.
gradle.taskGraph.whenReady {
    if (System.getenv("RNOTE_KEYSTORE") == null && allTasks.any { it.name.contains("Release") }) {
        throw GradleException("RNOTE_KEYSTORE is not set: see android/README.md, \"Signing\"")
    }
}
