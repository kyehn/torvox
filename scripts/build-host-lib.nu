#!/usr/bin/env -S nix develop --command nu

# 宿主 libnative.so 供 JVM 侧 JNI 冒烟测试加载（android/app/src/test/.../NativeBridgeSmokeTest.kt）。
# 测试固定加载 ../../target/release/libnative.so，缺失时自然失败，因此必须先于 gradle 门禁构建。

def main [] {
    cargo build --package native --profile release
}
