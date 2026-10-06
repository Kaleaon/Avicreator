pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "CharMorphAndroid"

include(
    ":app",
    ":core-model",
    ":asset-base",
    ":ingest-pipeline",
    ":ml-engine",
    ":native-bridge",
    ":storage",
    ":preview-renderer",
    ":feature-photo-import",
    ":libs:ktheme-compose",
    ":libs:ktheme-core",
)

val localKthemeCompose = file("libs/ktheme-compose")
val localKthemeCore = file("libs/ktheme-core")
val extKthemeCompose = file("../../Ktheme/libs/ktheme-compose")
val extKthemeCore = file("../../Ktheme/libs/ktheme-core")

project(":libs:ktheme-compose").projectDir = if (localKthemeCompose.exists()) localKthemeCompose else extKthemeCompose
project(":libs:ktheme-core").projectDir = if (localKthemeCore.exists()) localKthemeCore else extKthemeCore
