pluginManagement {
    repositories {
        // Tencent Cloud mirror of Maven Central (repo.maven.apache.org
        // rate-limits shared IPs with HTTP 429). Portal before mavenCentral:
        // some plugin markers (e.g. spotless) exist only on the portal, and
        // mavenCentral is only consulted when both miss.
        maven(url = "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/")
        google()
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // Tencent Cloud mirror of Maven Central first — repo.maven.apache.org
        // rate-limits shared IPs with HTTP 429. mavenCentral() is kept last as
        // a fallback for artifacts the mirror lacks; it is only consulted when
        // the earlier repositories miss.
        maven(url = "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/")
        google()
        mavenCentral()
    }
}

rootProject.name = "terminal"
include(":app")
include(":benchmark")
include(":baselineprofile")
