pluginManagement {
    repositories {
        gradlePluginPortal()
        maven(url = "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/")
        google()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        mavenCentral()
        google()
        maven(url = "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/")
    }
}

rootProject.name = "terminal"
include(":app")
include(":benchmark")
include(":baselineprofile")
