plugins {
    id("com.android.library")
    kotlin("android")
}

android {
    namespace = "com.charmorph.renderer"
    compileSdk = 34

    defaultConfig {
        minSdk = 26
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    aaptOptions {
        noCompress("filamat", "ktx")
    }
}

dependencies {
    implementation(project(":core-model"))
    implementation("androidx.core:core-ktx:1.13.1")

    // Filament
    val filamentVersion = "1.32.0"
    implementation("com.google.android.filament:filament-android:$filamentVersion")
    implementation("com.google.android.filament:utils-android:$filamentVersion")
    implementation("com.google.android.filament:gltfio-android:$filamentVersion")
    // Add Filamat for material compilation
    implementation("com.google.android.filament:filamat-android:$filamentVersion")
}
