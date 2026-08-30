# 验证报告: reference-adoption-v6

> 日期: 2026-08-30 | 状态: 计划中

## 验证范围

| 功能 | 验证方法 | 状态 |
|------|----------|------|
| mouse-encoding | Rust 单测 + 模拟器 | ⏳ 待实施 |
| a11y-overlay | Robolectric + 模拟器 | ⏳ 待实施 |
| osc133-semantic | Rust + Kotlin 单测 | ⏳ 待实施 |
| cell-run-cache | Rust 单测 + Benchmark | ⏳ 待实施 |

## 测试矩阵

### Rust 单测

| 测试文件 | 测试数 | 覆盖 |
|----------|--------|------|
| mouse_encoder.rs | ~16 | mode gate, SGR, bounds |
| output_processor.rs (OSC 133) | ~8 | A/B/C/D parsing |
| cell_builder.rs (CellRun) | ~4 | run detection |

### Kotlin 单测

| 测试文件 | 测试数 | 覆盖 |
|----------|--------|------|
| MouseModeTrackerTest | 8+ | 已有 |
| TerminalAccessibilityTest | ~4 | contentDescription |

### 模拟器集成测试

| 场景 | 验证方法 | 预期 |
|------|----------|------|
| vim 鼠标点击 | 手动 + `adb shell input mouse` | 光标移动 |
| TalkBack 朗读 | 手动 | 朗读可见内容 |
| `getLastCommandOutput` | 自动化 | 返回正确输出 |

## 验收标准

| ID | 标准 | 验证 |
|----|------|------|
| V1 | 所有现有测试通过 | `cargo test` + `gradle test` |
| V2 | 新增测试全部通过 | 同上 |
| V3 | 模拟器 90fps+ | `adb shell dumpsys gfxinfo` |
| V4 | 无新增 clippy warnings | `cargo clippy` |
| V5 | 无新增 lint errors | `gradle lint` |

## 回归检查

- [ ] PTY spawn/resize 正常
- [ ] 选择/复制/粘贴正常
- [ ] 搜索功能正常
- [ ] 修饰键栏正常
- [ ] 主题切换正常
- [ ] Bootstrap 安装正常
