plugins {
    id("com.android.application")
    id("org.jetbrains.dokka")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.dagger.hilt.android")
    id("com.google.devtools.ksp")
    id("io.github.takahirom.roborazzi")
    id("io.gitlab.arturbosch.detekt")
}

detekt {
    config.setFrom(files("../detekt.yml"))
    baseline = file("detekt-baseline.xml")
    buildUponDefaultConfig = true
    allRules = false
}

android {
    namespace = "io.torvox"
    compileSdk = 37

    signingConfigs {
        create("testkey") {
            storeFile = file("aosp-testkey.p12")
            storePassword = "android"
            keyAlias = "testkey"
            keyPassword = "android"
        }

        create("release") {
            val keystoreFile = System.getenv("ANDROID_KEYSTORE_FILE")
            val keystorePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
            val keyAlias = System.getenv("ANDROID_KEY_ALIAS")
            val keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            if (keystoreFile != null && keystorePassword != null) {
                storeFile = file(keystoreFile)
                storePassword = keystorePassword
                this.keyAlias = keyAlias ?: "release"
                this.keyPassword = keyPassword ?: keystorePassword
            }
        }
    }

    defaultConfig {
        applicationId = "com.termux"
        minSdk = 33
        targetSdk = 36
        versionCode = 2000
        versionName = "0.1.0"
        signingConfig = signingConfigs.getByName("testkey")

        val useCucumber = project.findProperty("cucumber")?.toString()?.toBoolean() ?: true
        testInstrumentationRunner =
            if (useCucumber) {
                "io.cucumber.android.runner.CucumberAndroidJUnitRunner"
            } else {
                "androidx.test.runner.AndroidJUnitRunner"
            }
        if (useCucumber) {
            testInstrumentationRunnerArguments["notCucumber"] = "true"
        }
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
            val hasKeystore = System.getenv("ANDROID_KEYSTORE_FILE") != null
            signingConfig = if (hasKeystore) signingConfigs.getByName("release") else signingConfigs.getByName("testkey")
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
    }

    sourceSets
        .named("main")
        .get()
        .jniLibs
        .directories
        .add("src/main/jniLibs")

    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.06.01")
    implementation(composeBom)

    implementation("androidx.core:core-ktx:1.19.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.11.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.activity:activity-compose:1.13.0")

    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.foundation:foundation")

    implementation("com.google.dagger:hilt-android:2.60")
    ksp("com.google.dagger:hilt-android-compiler:2.60")
    implementation("com.google.errorprone:error_prone_annotations:2.36.0")
    implementation("androidx.hilt:hilt-navigation-compose:1.3.0")

    implementation("androidx.datastore:datastore-preferences:1.1.3")
    implementation("androidx.navigation:navigation-compose:2.9.8")

    implementation("net.java.dev.jna:jna:5.19.1@aar")

    testImplementation("junit:junit:4.13.2")
    testImplementation("io.mockk:mockk:1.14.11")
    testImplementation("app.cash.turbine:turbine:1.2.1")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
    testImplementation("org.robolectric:robolectric:4.16.1")
    testImplementation(composeBom)
    testImplementation("androidx.compose.ui:ui-test-junit4")
    testImplementation("androidx.compose.ui:ui-test-manifest")
    testImplementation("io.github.takahirom.roborazzi:roborazzi:1.65.0")
    testImplementation("io.github.takahirom.roborazzi:roborazzi-compose:1.65.0")
    testImplementation("io.github.takahirom.roborazzi:roborazzi-junit-rule:1.65.0")
    testImplementation("androidx.test:core:1.7.0")
    testImplementation("com.tngtech.archunit:archunit-junit4:1.4.0")

    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.7.0")
    androidTestImplementation("androidx.test.espresso:espresso-contrib:3.7.0")
    androidTestImplementation("androidx.test.espresso:espresso-intents:3.7.0")
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test:rules:1.7.0")
    androidTestImplementation(composeBom)
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    androidTestImplementation("androidx.compose.ui:ui-test-manifest")
    androidTestImplementation("com.google.dagger:hilt-android-testing:2.60")
    kspAndroidTest("com.google.dagger:hilt-android-compiler:2.60")
    androidTestImplementation("com.google.mlkit:text-recognition:16.0.1")

    androidTestImplementation("io.cucumber:cucumber-android:7.18.1")

    androidTestImplementation("io.github.takahirom.roborazzi:roborazzi:1.65.0")
    androidTestImplementation("io.github.takahirom.roborazzi:roborazzi-compose:1.65.0")
    androidTestImplementation("io.github.takahirom.roborazzi:roborazzi-junit-rule:1.65.0")
}

val workingDirForCargo = rootProject.projectDir.parentFile!!
check(File(workingDirForCargo, "Cargo.toml").exists()) {
    "Cargo.toml not found at $workingDirForCargo"
}

tasks.matching { it.name == "testDebugUnitTest" }.configureEach {
}
