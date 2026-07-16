plugins {
    id("com.android.application") version "9.2.1" apply false
    id("com.android.library") version "9.2.1" apply false
    id("org.jetbrains.dokka") version "2.2.0" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.0" apply false
    id("com.google.dagger.hilt.android") version "2.60.1" apply false
    id("com.google.devtools.ksp") version "2.3.10" apply false
    id("io.github.takahirom.roborazzi") version "1.68.0" apply false
    id("com.diffplug.spotless") version "8.8.0" apply false
    id("io.gitlab.arturbosch.detekt") version "1.23.8" apply false
    id("androidx.benchmark") version "1.5.0-alpha06" apply false
    id("androidx.baselineprofile") version "1.5.0-alpha06" apply false
    id("com.github.ben-manes.versions") version "0.54.0" apply false
}

val verrors = mutableListOf<String>()

// Check 1 - GC root (NIX_GCROOT).
// Set ONLY by nix develop / nix-shell - NOT by nix build.
val gcroot = System.getenv("NIX_GCROOT")
if (gcroot.isNullOrEmpty()) {
    verrors.add("NIX_GCROOT is not set (nix develop sets this; nix build does not)")
} else {
    if (!gcroot.matches(Regex("^/nix/store/[0-9a-z]{32}[-].+"))) {
        verrors.add("NIX_GCROOT = '$gcroot' - invalid store path format")
    }
    if (!File(gcroot).exists()) {
        verrors.add("NIX_GCROOT = '$gcroot' - path does not exist on disk")
    }
}

// Check 2 - IN_NIX_SHELL flag + NIX_BUILD_TOP pattern.
// IN_NIX_SHELL is set ONLY by nix develop (not nix build).
// NIX_BUILD_TOP starts with /tmp/nix-shell. in develop, /tmp/nix-build- in build.
val nixShell = System.getenv("IN_NIX_SHELL")
if (nixShell != "1" && nixShell != "impure") {
    verrors.add("IN_NIX_SHELL = '$nixShell' - expected '1' or 'impure' (nix develop only)")
}

val nixBuildTop = System.getenv("NIX_BUILD_TOP")
if (nixBuildTop.isNullOrEmpty()) {
    verrors.add("NIX_BUILD_TOP is not set (nix develop creates a temp dir)")
} else if (!nixBuildTop.contains("/nix-shell.")) {
    verrors.add("NIX_BUILD_TOP = '$nixBuildTop' - expected /(mnt/)?tmp/nix-shell.* (nix develop creates a temp dir)")
}

// Check 3 - PATH / nativeBuildInputs cross-reference.
// nix develop prepends /nix/store/<hash>/bin entries to PATH.
val path = System.getenv("PATH") ?: ""
val nixBins = path.split(":").filter { it.startsWith("/nix/store/") }
if (nixBins.isEmpty()) {
    verrors.add("PATH has no /nix/store/ entries (nix develop prepends these)")
} else {
    for (bin in nixBins) {
        val parent = bin.removeSuffix("/bin")
        if (!parent.matches(Regex("^/nix/store/[0-9a-z]{32}[-].+"))) {
            verrors.add("PATH entry '$bin' has invalid nix store format")
        }
    }
    val nativeInputs = System.getenv("nativeBuildInputs") ?: ""
    if (nativeInputs.isNotEmpty()) {
        val firstBin = nixBins.first().removeSuffix("/bin")
        if (!nativeInputs.contains(firstBin)) {
            verrors.add("First nix PATH entry '$firstBin' not found in nativeBuildInputs")
        }
    }
}

if (verrors.isNotEmpty()) {
    logger.error("=== nix develop environment check failed ===")
    verrors.forEach { logger.error("  X $it") }
    logger.error("")
    logger.error("Run: nix develop")
    logger.error("")
    throw GradleException("Must run inside nix develop (${verrors.size} check(s) failed)")
}

subprojects {
    apply(plugin = "com.diffplug.spotless")
    configure<com.diffplug.gradle.spotless.SpotlessExtension> {
        kotlin {
            ktlint("1.8.0")
                .editorConfigOverride(
                    mapOf(
                        "ktlint_function_naming_ignore_when_annotated_with" to "Composable",
                    ),
                )
            target("src/**/*.kt")
            targetExclude("**/build/**")
        }
        kotlinGradle {
            ktlint().editorConfigOverride(mapOf("max_line_length" to "300"))
            target("*.gradle.kts")
        }
    }
}

// Gradle Versions Plugin: provides `dependencyUpdates` task at root level
apply(plugin = "com.github.ben-manes.versions")
