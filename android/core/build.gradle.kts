// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
plugins { id("com.android.library") }

android {
    namespace = "io.github.creatorkoo.roamling.core"
    compileSdk { version = release(37) { minorApiLevel = 1 } }
    buildToolsVersion = "37.0.0"
    ndkVersion = "30.0.16248370"
    defaultConfig { minSdk = 30 }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

val verifyNativeArtifacts = tasks.register("verifyNativeArtifacts") {
    doLast {
        for (abi in listOf("arm64-v8a", "x86_64")) {
            check(file("src/main/jniLibs/$abi/libroamling_android.so").isFile) {
                "Missing $abi Rust library. Run scripts/build-android-core.ps1 or .sh first."
            }
        }
        for (component in listOf("roamling_core", "roamling_android")) {
            check(file("src/main/kotlin/uniffi/$component/$component.kt").isFile) {
                "Missing $component Kotlin bindings. Run scripts/build-android-core.ps1 or .sh first."
            }
        }
    }
}
tasks.named("preBuild") { dependsOn(verifyNativeArtifacts) }

dependencies {
    implementation("androidx.annotation:annotation:1.10.0")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
}
