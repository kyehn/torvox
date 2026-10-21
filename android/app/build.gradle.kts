plugins {
  id("com.android.application")
  id("androidx.baselineprofile")
  id("org.jetbrains.dokka")
  id("org.jetbrains.kotlin.plugin.compose")
  id("org.jetbrains.kotlin.plugin.serialization")
  id("com.google.dagger.hilt.android")
  id("com.google.devtools.ksp")
  id("dev.detekt")
  id("com.ncorti.ktfmt.gradle")
  id("org.jlleitschuh.gradle.ktlint")
}

detekt {
  config.setFrom(files("../detekt.yml"))
  buildUponDefaultConfig = true
  allRules = false
}

dependencies {
  detektPlugins("dev.detekt:detekt-rules-ktlint-wrapper:2.0.0-alpha.6")
  detektPlugins("dev.detekt:detekt-rules-libraries:2.0.0-alpha.6")
  detektPlugins("dev.detekt:detekt-rules-ruleauthors:2.0.0-alpha.6")
}

android {
  namespace = "terminal.emulator"
  compileSdk = 37

  signingConfigs {
    create("testkey") {
      storeFile = file("aosp-testkey.p12")
      storePassword = "android"
      keyAlias = "testkey"
      keyPassword = "android"
    }
  }

  defaultConfig {
    applicationId = "com.termux"
    minSdk = 33
    targetSdk = 28
    versionCode = 2000
    versionName = "0.1.0"
    signingConfig = signingConfigs.getByName("testkey")

    testInstrumentationRunner = "io.cucumber.android.runner.CucumberAndroidJUnitRunner"
    testInstrumentationRunnerArguments["cucumberUseAndroidJUnitRunner"] = "true"
    testInstrumentationRunnerArguments["notClass"] = "com.termux.test.CucumberOptionsClass"
    ndk {
      abiFilters += listOf("arm64-v8a", "x86_64")
    }
  }

  buildTypes {
    debug {
      isMinifyEnabled = false
      isDebuggable = true
      signingConfig = signingConfigs.getByName("testkey")
    }
    release {
      isMinifyEnabled = true
      isShrinkResources = true
      signingConfig = signingConfigs.getByName("testkey")
      proguardFiles(
        getDefaultProguardFile("proguard-android-optimize.txt"),
        "proguard-rules.pro",
      )
    }
  }

  publishing {
    singleVariant("release")
  }

  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }

  buildFeatures {
    compose = true
    buildConfig = true
  }

  testOptions {
    unitTests.isIncludeAndroidResources = true
    unitTests.isReturnDefaultValues = true
  }

  sourceSets.named("main").get().jniLibs.directories.add("src/main/jniLibs")

  packaging {
    jniLibs {
      useLegacyPackaging = true
      keepDebugSymbols += listOf("**/libnative.so")
    }
  }
}

baselineProfile {
  automaticGenerationDuringBuild = false
  saveInSrc = true
}

configurations {
  all {
    resolutionStrategy {
      force("androidx.concurrent:concurrent-futures:1.3.0")
    }
  }
}

dependencies {
  val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
  implementation(composeBom)

  implementation("androidx.core:core-ktx:1.19.1")
  implementation("androidx.core:core-splashscreen:1.2.0")
  implementation("org.jetbrains.kotlinx:kotlinx-collections-immutable:0.5.2")
  implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.12.0-alpha04")
  implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.12.0-alpha04")
  implementation("androidx.lifecycle:lifecycle-runtime-compose:2.12.0-alpha04")
  implementation("androidx.activity:activity-compose:1.14.0-alpha03")

  implementation("androidx.compose.ui:ui")
  implementation("androidx.compose.ui:ui-graphics")
  implementation("androidx.compose.material3:material3")
  implementation("androidx.compose.material:material-icons-extended")
  implementation("androidx.compose.foundation:foundation")

  implementation("com.google.dagger:hilt-android:2.60.1")
  ksp("com.google.dagger:hilt-android-compiler:2.60.1")
  implementation("androidx.hilt:hilt-navigation-compose:1.4.0")

  implementation("androidx.datastore:datastore-preferences:1.3.0-rc01")

  implementation("com.squareup.okhttp3:okhttp:5.5.0")
  testImplementation("com.squareup.okhttp3:mockwebserver3:5.5.0")
  testImplementation("com.squareup.okhttp3:okhttp-tls:5.5.0")
  debugImplementation("com.squareup.leakcanary:leakcanary-android:3.0-alpha-9")

  testImplementation("junit:junit:4.13.2")
  testImplementation("io.mockk:mockk:1.14.11")
  testImplementation("app.cash.turbine:turbine:1.2.1")
  testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")

  implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
  implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.12.0-RC")

  releaseImplementation("androidx.profileinstaller:profileinstaller:1.4.1")
  testImplementation("org.robolectric:robolectric:4.17")
  testImplementation(composeBom)
  testImplementation("androidx.compose.ui:ui-test-junit4")
  testImplementation("androidx.compose.ui:ui-test-manifest")
  testImplementation("androidx.test:core:1.7.0")

  debugImplementation("com.ms-square:debugoverlay:2.7.1")

  lintChecks("com.slack.lint.compose:compose-lint-checks:1.6.1")
  lintChecks("com.slack.lint:slack-lint-checks:0.11.1")

  androidTestImplementation("androidx.test.ext:junit:1.3.0")
  androidTestImplementation("androidx.test.espresso:espresso-core:3.7.0")
  androidTestImplementation("androidx.test.espresso:espresso-contrib:3.7.0")
  androidTestImplementation("androidx.test.espresso:espresso-intents:3.7.0")
  androidTestImplementation("androidx.test.uiautomator:uiautomator:2.4.0")
  androidTestImplementation("androidx.test:runner:1.7.0")
  androidTestImplementation("androidx.test:rules:1.7.0")
  androidTestImplementation(composeBom)
  androidTestImplementation("androidx.compose.ui:ui-test-junit4")
  androidTestImplementation("androidx.compose.ui:ui-test-manifest")
  androidTestImplementation("com.google.dagger:hilt-android-testing:2.60.1")
  kspAndroidTest("com.google.dagger:hilt-android-compiler:2.60.1")

  androidTestImplementation("io.cucumber:cucumber-android:7.19.0")

  baselineProfile(project(":baselineprofile"))
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
  compilerOptions {
    allWarningsAsErrors.set(true)
  }
}

tasks
  .withType<Test>()
  .matching { it.name == "testDebugUnitTest" }
  .configureEach {
    jvmArgs("-Djava.library.path=")
  }

val reportConnectedFailures = tasks.register("reportConnectedFailures") {
  description = "Prints connected-test failure names and messages from UTP XML results."
  doLast {
    val resultsDir =
      layout.buildDirectory
        .dir("outputs/androidTest-results/connected")
        .get()
        .asFile
    val reports = resultsDir.walkTopDown().filter { it.isFile && it.extension == "xml" }.toList()
    // 零用例 = 这一轮什么都没跑（APK 安装失败、用例过滤器写错、UTP 没连上设备）。
    // AGP 对「runner 启动前就失败」不会把主任务标红——实测 adb install 找不到 APK
    // 时 `connectedDebugAndroidTest` 仍报 BUILD SUCCESSFUL、只把异常塞进输出。
    // 那会让 CI 在一条测试都没跑的情况下静默变绿，故这里必须抛错；
    // docs/specification/TESTING.md 明确要求缺依赖时失败而不是跳过。
    if (reports.isEmpty()) {
      throw GradleException("connected tests produced no UTP XML results under ${resultsDir.path}")
    }
    var failed = 0
    var executed = 0
    val documentBuilder = javax.xml.parsers.DocumentBuilderFactory.newInstance().newDocumentBuilder()
    reports.forEach { report ->
      val document = documentBuilder.parse(report)
      val cases = document.getElementsByTagName("testcase")
      executed += cases.length
      for (index in 0 until cases.length) {
        val testCase = cases.item(index) as org.w3c.dom.Element
        val failures = testCase.getElementsByTagName("failure")
        val errors = testCase.getElementsByTagName("error")
        val detail = if (failures.length > 0) {
          failures.item(0)
        } else if (errors.length > 0) {
          errors.item(0)
        } else {
          null
        }
        if (detail != null) {
          failed++
          println("connected-failure: ${testCase.getAttribute("classname")}#${testCase.getAttribute("name")}")
          println("connected-failure-message: ${detail.textContent.trim().take(600)}")
        }
      }
    }
    println("connected-failures: $failed failed in ${reports.size} report files, $executed cases executed")
    if (executed == 0) {
      throw GradleException("connected tests executed 0 cases; nothing ran. Results: ${resultsDir.path}")
    }
  }
}

tasks
  .matching { it.name == "connectedDebugAndroidTest" }
  .configureEach {
    finalizedBy(reportConnectedFailures)
  }
