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

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
