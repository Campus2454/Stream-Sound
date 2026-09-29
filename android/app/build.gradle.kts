plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "app.streamsound"
    compileSdk = 34

    defaultConfig {
        applicationId = "app.streamsound"
        // Android 10 is the first version that can capture other apps' audio.
        minSdk = 29
        targetSdk = 34
        // CI passes the release version (.github/version.sh): "X.Y" official,
        // "X.Y.Z" beta. versionCode follows the same order (0.2 < 0.2.1 < 0.3),
        // so every release installs over the last; old installs had 1..14.
        val ver = System.getenv("SSND_VERSION")?.takeIf { Regex("""\d{1,2}\.\d{1,2}(\.\d{1,5})?""").matches(it) }
        val parts = ver?.split('.')?.map { it.toInt() }
        versionCode = if (parts == null) 1 else parts[0] * 10_000_000 + parts[1] * 100_000 + parts.getOrElse(2) { 0 }
        versionName = ver ?: "dev"
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    // Fixed key for sideloading, so a new build installs over the old one.
    // Personal use only; this is not a secret.
    signingConfigs {
        create("sideload") {
            storeFile = file("sideload.keystore")
            storePassword = "streamsound"
            keyAlias = "streamsound"
            keyPassword = "streamsound"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("sideload")
        }
        debug {
            signingConfig = signingConfigs.getByName("sideload")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    lint {
        // Personal sideload build: lint findings are reported, never block the apk.
        abortOnError = false
        checkReleaseBuilds = false
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.09.02")
    implementation(composeBom)
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.2")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
}
