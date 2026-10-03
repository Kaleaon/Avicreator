// Root Gradle build script for the CharMorph Android ingestion project.

plugins {
    id("com.android.application") version "8.5.0" apply false
    id("com.android.library") version "8.5.0" apply false
    kotlin("android") version "1.9.24" apply false
    kotlin("kapt") version "1.9.24" apply false
    kotlin("plugin.serialization") version "1.9.24" apply false
}

tasks.register("clean", Delete::class) {
    delete(rootProject.buildDir)
}
