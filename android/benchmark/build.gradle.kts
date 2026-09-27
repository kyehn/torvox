plugins {
  id("com.android.test")
  id("androidx.baselineprofile")
}

android {
  namespace = "terminal.emulator.benchmark"
  compileSdk = 37
  targetProjectPath = ":app"

  defaultConfig {
    minSdk = 33
    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    testInstrumentationRunnerArguments["androidx.benchmark.suppressErrors"] = "EMULATOR"
  }

  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
}

dependencies {
  implementation("androidx.benchmark:benchmark-macro-junit4:1.5.0")
  implementation("androidx.test.ext:junit:1.3.0")
  implementation("androidx.test.uiautomator:uiautomator:2.4.0")
  implementation("androidx.test:runner:1.7.0")
}
