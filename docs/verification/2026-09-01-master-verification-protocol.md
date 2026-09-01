# Master Verification Protocol v8

> 日期：2026-09-01 | 状态：活跃

## 验证项（S1-S9，对应 proposal.md 成功判定）

### S1: Rust 单元测试全绿

```bash
cargo test -p native --lib 2>&1 | grep -E "test result:|passed"
# 期望：995+ passed, 0 failed, 32 ignored (GPU-dependent)
```

### S2: Clippy 零新增告警

```bash
cargo clippy -p native -- -D warnings 2>&1 | grep -c "warning"
# 期望：0
```

### S3: 未用依赖零

```bash
cargo machete 2>&1 | grep -c "unused"
# 期望：0
```

### S4: Release .so 体积守卫

```bash
# 交叉编译（需要 NDK）
cargo ndk -t arm64-v8a build --release --lib -p native 2>&1
ls -lh target/aarch64-linux-android/release/libnative.so
readelf --dynamic target/aarch64-linux-android/release/libnative.so | grep NEEDED
# 期望：14-18M，无 NEEDED ghostty
```

### S5: APK 含 .so

```bash
./gradlew :app:assembleDebug 2>&1 | tail -5
find android/app/build/outputs/apk/debug/ -name "*.apk" -exec unzip -l {} \; | grep libnative.so
# 期望：包含 lib/arm64-v8a/libnative.so
```

### S6: 模拟器帧率

```bash
# 启动应用
adb shell am start -n com.terminal.emulator/.TerminalActivity
sleep 5  # 等待渲染稳定

# 采集帧率
adb shell dumpsys gfxinfo com.terminal.emulator framestats > /tmp/framestats.txt
# 解析 90th percentile
# 期望：90th < 16ms（即 90fps+）
```

### S7: 依赖滚动零破坏

```bash
cargo update 2>&1 | tail -3
cargo test -p native --lib 2>&1 | grep -E "test result:"
# 期望：995+ passed, 0 failed, 32 ignored (GPU-dependent)
```

### S8: 连续三次审阅零阻塞

```bash
# 三轮自动 review
# 第1轮 → 第2轮 → 第3轮
# 期望：连续 3 轮零 P0/P1 阻塞
```

### S9: 代码量净减少

```bash
git diff --stat HEAD~10..HEAD | grep "files changed"
# 期望：net LoC ≤ 0
```

## 模拟器环境准备

```bash
# 启动 API 35 模拟器
emulator -avd Pixel_7_API_35 -no-window -gpu swiftshader_indirect &
adb wait-for-device
adb shell getprop ro.build.version.sdk  # 应返回 35

# 安装 APK
adb install -r android/app/build/outputs/apk/debug/app-debug.apk

# 验证 Vulkan 可用
adb shell getprop ro.hardware.vulkan  # 应返回 "swiftshader" 或 "android"
```

## 执行顺序

1. **S1-S3**（本地，每次提交）→ **S4-S5**（构建验证）→ **S7**（滚动验证）
2. **S6**（模拟器帧率）→ **S8**（三轮审阅）→ **S9**（代码量）
3. 全部通过后产出 `docs/verification/YYYY-MM-DD-master-v8-verification.md`

## 异常处理

| 异常 | 处理 |
|------|------|
| S1 某测试 flaky | 标记 `#[ignore]` + 建 issue，不计入基线 |
| S4 .so 超 18M | 检查 `cargo tree -e features` 精简 feature |
| S6 帧率 <90fps | 检查 GPU 加速设置、关闭 background apps |
| S8 审阅发现问题 | 修复后重置计数器，重新 3 轮 |
