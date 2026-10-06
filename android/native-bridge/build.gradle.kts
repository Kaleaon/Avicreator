plugins {
    id("com.android.library")
    kotlin("android")
}

android {
    namespace = "com.charmorph.nativebridge"
    compileSdk = 34

    defaultConfig {
        minSdk = 26
        externalNativeBuild {
            cmake {
                cppFlags("")
            }
        }
    }

    externalNativeBuild {
        cmake {
            path("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation(project(":core-model"))
    implementation("androidx.core:core-ktx:1.13.1")
}
