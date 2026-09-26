plugins {
  id("com.android.application") version "9.5.0-alpha07" apply false
  id("com.android.library") version "9.5.0-alpha07" apply false
  id("org.jetbrains.dokka") version "2.3.0-Beta" apply false
  id("org.jetbrains.kotlin.plugin.compose") version "2.4.20" apply false
  id("org.jetbrains.kotlinx.kover") version "0.9.9" apply false
  id("org.jetbrains.kotlin.plugin.serialization") version "2.4.20" apply false
  id("com.google.dagger.hilt.android") version "2.60.1" apply false
  id("com.google.devtools.ksp") version "2.3.12" apply false
  id("com.diffplug.spotless") version "8.10.3" apply false
  id("dev.detekt") version "2.0.0-alpha.6" apply false
  id("androidx.benchmark") version "1.5.0" apply false
  id("androidx.baselineprofile") version "1.5.0" apply false
  id("io.github.ben-manes.versions") version "0.64.0" apply false
  id("com.ncorti.ktfmt.gradle") version "0.27.0" apply false
  id("org.jlleitschuh.gradle.ktlint") version "14.2.0" apply false
  id("de.infix.testBalloon") version "1.1.0" apply false
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
                  // 与 detekt ktlint-wrapper（同为 ktlint 1.8.0，code_style intellij_idea 默认列宽 120）保持一致，
                  // 否则两工具在表达式函数体换行上互斥，fmt 无法收敛。
                  "max_line_length" to "120",
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

apply(plugin = "io.github.ben-manes.versions")
