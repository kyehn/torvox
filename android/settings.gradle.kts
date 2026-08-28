pluginManagement {
    repositories {
        // gradlePluginPortal() first: the Tencent Cloud Maven proxy returns
        // inconsistent responses for plugin artifacts absent from its index
        // (HTTP 200 + 337-byte stub JAR on plain GET, 404 + HTML on Accept
        // headers), which prevents Gradle from falling through to the next
        // repo.  gradlePluginPortal() is the authoritative source for Gradle
        // plugin artifacts; the mirror + google() + mavenCentral() cover
        // transitive Maven dependencies.
        gradlePluginPortal()
        maven(url = "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/")
        google()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // mavenCentral() + google() first: same Tencent Cloud mirror issue
        // as pluginManagement above — inconsistent responses block fallback.
        mavenCentral()
        google()
        maven(url = "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/")
    }
}

rootProject.name = "terminal"
include(":app")
include(":benchmark")
include(":baselineprofile")
