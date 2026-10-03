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
)
