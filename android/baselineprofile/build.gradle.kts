plugins {
    id("com.android.library")
    id("androidx.baselineprofile")
}

android {
    namespace = "io.torvox.baselineprofile"
    compileSdk = 37

    defaultConfig {
        minSdk = 33
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    compileOnly(project(":app"))
    androidTestImplementation("androidx.benchmark:benchmark-macro-junit4:1.5.0-alpha06")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.7.0")
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
}

baselineProfile {
    automaticGenerationDuringBuild = false
    saveInSrc = true
}
