plugins {
  id("com.android.test")
  id("androidx.baselineprofile")
}

android {
  namespace = "terminal.emulator.baselineprofile"
  compileSdk = 37
  targetProjectPath = ":app"

  defaultConfig {
    minSdk = 31
    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
  }

  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
}

dependencies {
  ktlint("io.github.detekt.sarif4k:sarif4k:0.8.0")
  ktlint("io.github.oshai:kotlin-logging:8.0.7")
  implementation("androidx.benchmark:benchmark-macro-junit4:1.6.0-alpha02")
  implementation("androidx.test.ext:junit:1.4.0-alpha04")
  implementation("androidx.test.uiautomator:uiautomator:2.4.1")
  implementation("androidx.test:runner:1.8.0-alpha04")
}
