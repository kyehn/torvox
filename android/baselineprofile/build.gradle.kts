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
  implementation("androidx.benchmark:benchmark-macro-junit4:1.6.0-alpha02")
  implementation("androidx.test.ext:junit:1.4.0-alpha04")
  implementation("androidx.test.uiautomator:uiautomator:2.4.1")
  implementation("androidx.test:runner:1.8.0-alpha04")
}
