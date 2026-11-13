plugins {
  id("com.android.test")
  id("androidx.baselineprofile")
}

android {
  namespace = "terminal.emulator.benchmark"
  compileSdk = 37
  targetProjectPath = ":app"

  // Nix 变种（包名 com.termux.nix）的引导包安装用例需要一个指向该变种的测试包：
  // 仪器测试的宿主包名由被测变种决定，故这里加一个同名构建类型。
  buildTypes {
    create("nix") {
      initWith(getByName("debug"))
      matchingFallbacks += listOf("debug")
    }
  }

  defaultConfig {
    minSdk = 31
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
  implementation("androidx.test.uiautomator:uiautomator:2.4.1")
  implementation("androidx.test:runner:1.7.0")
}
