// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
plugins { id("com.android.application") }

// One release version, owned by the Rust workspace. Bound the components so
// distinct semantic versions cannot map to the same Android versionCode.
val cargoManifest = providers.fileContents(rootProject.layout.projectDirectory.file("../rust/Cargo.toml")).asText.get()
val appVersion = Regex("(?m)^version = \"(\\d+\\.\\d+\\.\\d+)\"$")
    .find(cargoManifest)?.groupValues?.get(1) ?: error("Missing Rust workspace version")
val (major, minor, patch) = appVersion.split('.').map(String::toInt)
require(major in 0..2099 && minor in 0..999 && patch in 0..999)

android {
    namespace = "io.github.creatorkoo.roamling"
    compileSdk { version = release(37) { minorApiLevel = 1 } }
    buildToolsVersion = "37.0.0"
    ndkVersion = "30.0.16248370"
    defaultConfig {
        applicationId = "io.github.creatorkoo.roamling"
        minSdk = 30
        targetSdk = 37
        versionName = appVersion
        versionCode = major * 1_000_000 + minor * 1_000 + patch
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(project(":core"))
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test:core:1.7.0")
    androidTestImplementation("junit:junit:4.13.2")
}
