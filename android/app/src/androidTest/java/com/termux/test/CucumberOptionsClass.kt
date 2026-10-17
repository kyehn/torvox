package com.termux.test

import io.cucumber.junit.CucumberOptions
import terminal.emulator.cucumber.SimpleHiltObjectFactory

/**
 * Cucumber options class for the test APK package.
 * The Cucumber runner scans com.termux.test for @CucumberOptions.
 *
 * @Suppress("DEPRECATION") 在这里是唯一可行的处理：上游弃用 `@CucumberOptions` 的理由是
 * 「JUnit 4 进入维护模式，建议改用 JUnit 5 + cucumber-junit-platform-engine」，而
 * cucumber-android 的 `CucumberJunitRunner` 只跑在 JUnit 4 的 `AndroidJUnitRunner` 上；
 * 它还会扫描本包并 `throw CucumberException("No CucumberOptions annotated class present in
 * package ...")`，即这个标注类不是可选项而是硬性要求，无法迁移。上游自己的
 * `CucumberJunitRunner.kt` 首行同样是 `@file:Suppress("DEPRECATION") // Android uses Junit4`，
 * 这里是同一条理由的站点级写法，不掩盖其它告警（`allWarningsAsErrors` 仍然全开）。
 */
@Suppress("DEPRECATION")
@CucumberOptions(
    glue = ["terminal.emulator.cucumber"],
    features = ["features"],
    plugin = ["pretty"],
    objectFactory = SimpleHiltObjectFactory::class,
)
class CucumberOptionsClass
