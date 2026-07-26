package io.term.architecture

import com.tngtech.archunit.junit.AnalyzeClasses
import com.tngtech.archunit.junit.ArchTest
import com.tngtech.archunit.lang.ArchRule
import com.tngtech.archunit.library.Architectures.layeredArchitecture

@AnalyzeClasses(packages = ["io.term"])
object LayerDependencyTest {
    private const val UI = "UI"
    private const val BRIDGE = "Bridge"
    private const val RUNTIME = "Runtime"
    private const val SERVICE = "Service"
    private const val SETTINGS = "Settings"
    private const val INSTALLER = "Installer"
    private const val EXEC = "Exec"

    @ArchTest
    val layerDependencies: ArchRule =
        layeredArchitecture()
            .consideringAllDependencies()
            .layer(UI)
            .definedBy("io.term.ui..")
            .layer(BRIDGE)
            .definedBy("io.term.bridge..")
            .layer(RUNTIME)
            .definedBy("io.term.runtime..")
            .layer(SERVICE)
            .definedBy("io.term.service..")
            .layer(SETTINGS)
            .definedBy("io.term.settings..")
            .layer(INSTALLER)
            .definedBy("io.term.installer..")
            .layer(EXEC)
            .definedBy("io.term.exec..")
            .whereLayer(UI)
            .mayOnlyBeAccessedByLayers(UI)
            .whereLayer(BRIDGE)
            .mayOnlyBeAccessedByLayers(UI, RUNTIME, SERVICE)
            .whereLayer(RUNTIME)
            .mayOnlyBeAccessedByLayers(UI, SERVICE)
            .whereLayer(SERVICE)
            .mayOnlyBeAccessedByLayers(UI)
            .whereLayer(SETTINGS)
            .mayOnlyBeAccessedByLayers(UI, RUNTIME)
            .whereLayer(INSTALLER)
            .mayOnlyBeAccessedByLayers(UI, RUNTIME, EXEC)
            .whereLayer(EXEC)
            .mayOnlyBeAccessedByLayers(UI, RUNTIME)
}
