plugins {
  id("com.android.application") version "9.4.0" apply false
  id("com.android.library") version "9.4.0" apply false
  id("org.jetbrains.dokka") version "2.2.0" apply false
  id("org.jetbrains.kotlin.plugin.compose") version "2.4.20" apply false
  id("org.jetbrains.kotlinx.kover") version "0.9.9" apply false
  id("org.jetbrains.kotlin.plugin.serialization") version "2.4.20" apply false
  id("com.google.dagger.hilt.android") version "2.60.1" apply false
  id("com.google.devtools.ksp") version "2.3.12" apply false
  id("com.diffplug.spotless") version "8.10.2" apply false
  id("io.gitlab.arturbosch.detekt") version "1.23.8" apply false
  id("androidx.benchmark") version "1.5.0" apply false
  id("androidx.baselineprofile") version "1.5.0" apply false
  id("com.github.ben-manes.versions") version "0.61.0" apply false
  id("com.ncorti.ktfmt.gradle") version "0.27.0" apply false
  id("org.jlleitschuh.gradle.ktlint") version "14.2.0" apply false
  id("de.infix.testBalloon") version "1.0.1-K2.4.0" apply false
}

if (
    System.getenv("NIX_GCROOT").isNullOrEmpty() ||
        System.getenv("IN_NIX_SHELL").isNullOrEmpty() ||
        System.getenv("NIX_BUILD_TOP").isNullOrEmpty() ||
        System.getenv("nativeBuildInputs").isNullOrEmpty()
) {
  throw GradleException("Must run inside nix develop")
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
      ktlint()
          .editorConfigOverride(
              mapOf(
                  "max_line_length" to "300",
                  "indent_size" to "2",
                  "indent_style" to "space",
              ),
          )
      target("*.gradle.kts")
    }
  }

  apply(plugin = "org.jlleitschuh.gradle.ktlint")
  configure<org.jlleitschuh.gradle.ktlint.KtlintExtension> {
    version.set("1.8.0")
    android.set(true)
  }
}

// Gradle Versions Plugin: provides `dependencyUpdates` task at root level
apply(plugin = "com.github.ben-manes.versions")
