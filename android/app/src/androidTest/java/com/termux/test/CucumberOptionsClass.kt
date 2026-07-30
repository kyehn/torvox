package com.termux.test

import io.cucumber.junit.CucumberOptions
import terminal.emulator.cucumber.SimpleHiltObjectFactory

/**
 * Cucumber options class for the test APK package.
 * The Cucumber runner scans com.termux.test for @CucumberOptions.
 */
@CucumberOptions(
    glue = ["terminal.emulator.cucumber"],
    features = ["features"],
    tags = "not @wip",
    plugin = ["pretty"],
    objectFactory = SimpleHiltObjectFactory::class,
)
class CucumberOptionsClass
