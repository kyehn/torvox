#!/usr/bin/env -S nix develop --command nu

# Gradle 侧格式化与静态检查自动修复。fmt 工作流调用，禁止在 workflow 内直接执行 gradlew。

def main [] {
    cd android
    ^./gradlew spotlessApply
    ^./gradlew detekt --auto-correct
}
