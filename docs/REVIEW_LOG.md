# 全量代码审查记录

## 范围与依据

- 审查对象：全仓现行代码，不以某次 diff 或提交为界；本轮覆盖 145 个 Rust 文件（53,413 行）与 115 个 Kotlin 文件（30,334 行）。
- 外部规则：`awesome-skills/code-review-skill` 的 `SKILL.md`、`review-checklist.md`、`code-quality-universal.md`、`common-bugs-checklist.md`、`architecture-review-guide.md`、`performance-review-guide.md`、`security-review-guide.md`、`kotlin.md`、`rust.md`；`android/skills` 的 `android-jni-security`、`jetpack-compose-performance`、`anr-audit-and-fix`、`r8-analyzer`、`edge-to-edge`、`testing-setup`、`agp-9-upgrade`。
- 本地规则：`docs/specification/` 全部文档，以及 `rust-review`、`rust-best-practices`、`code-review-and-quality`、`compose-expert`、`compose-screens-footguns`、`state-and-background-footguns`、`data-layer-footguns`、`security-and-hardening`。
- 重点模块：Rust FFI/JNI 边界、会话与 PTY、渲染/Kitty 图集、字体与日志；Kotlin 启动与 ViewModel、渲染监督、Compose UI、设置/DataStore、DocumentsProvider、引导下载与解压、构建与 R8。

## 发现与处置

| 提交 | 模块 | 问题 | 处置 |
| --- | --- | --- | --- |
| `8c410a10`、`f2241f32`、`77d87ad1` | 引导安装 | 解压条目循环与单个大条目写入循环不检查取消，用户取消后仍继续写满 staging | 两处循环均检查 `shouldAbort()` 并抛 `CancellationException`；规范同步 |
| `ef825a21` | DocumentsProvider | 根路径包含判断用字符串前缀，`..foo` 等同级目录可能被误判为在根内 | 统一按路径元素比较的 `isInsideRoot` |
| `3823c25b` | DocumentsProvider | 回滚产生的占位文件出现在搜索结果 | 搜索过滤回滚占位 |
| `c86c4ea3` | 渲染监督 | Effect 取消时 render-pause token 丢失，可能永久暂停渲染 | token 跨 Effect 取消保留 |
| `027c75ba` | 渲染 | GPU readback map 超时过短，并行构建/测试下偶发失败 | 放宽超时 |
| `c6da204d` | Kitty 图集 | 图集仅按软件预算校验，可能超过设备 `max_texture_dimension_2d` | 打包、借用、复用三条路径统一按设备上限与预算双重拒绝；规范同步 |
| `b6e4ee89`、`df53dcd3` | R8 | `-keep class terminal.emulator.** { *; }` 使 JNI 入口与全部 Kotlin 类免于 shrink | 收窄为仅保留必要的 `keepclassmembers`；R8 Analyzer 分数由 87.50% 提升至 98.70%；说明不可达 native 方法为何可安全移除 |
| `dcae6999` | JNI | `listFontFamilies` 与批量会话查询在循环中创建局部引用却不释放，长列表可能耗尽局部引用表 | 立即释放临时 class/string 局部引用 |
| `ea27fe99`、`e7b274dd` | 终端抽取 | 逐格分配与缺行补齐的成本未留档 | 补齐取舍台账并用测试钉住分配预算 |
| `834b8b82` | KGP | 单图借用分支绕过条带路径预算判据 | 两路径共用预算判据 |

重复度扫描（`jscpd`，阈值 8 行/80 token）：Rust 0 处克隆；Kotlin 1 处 11 行重复，且为两个设置分区的相同 import 块，不构成逻辑重复。

## 验证

- Rust：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all-targets` 为 982 + 1 + 2 + 1 个测试全部通过。
- Android：`nu scripts/build-android-libs.nu x86_64` 后执行 `detekt`、`spotlessCheck`、`app:dokkaGenerate`、`lintDebug`、`lintVitalRelease`、`assembleDebugAndroidTest`、`testDebugUnitTest`（1166 个单元测试）、benchmark 与 baselineprofile 编译、`assembleRelease`、`analyzeReleaseR8Config`，全部 `BUILD SUCCESSFUL`。
- 反汇编 release `classes.dex`：69 个可达 JNI 方法全部保留；9 个仅诊断/测试用且 Kotlin 侧无可达调用点的方法随死代码移除，原生侧不存在通过 JNI 反射回调 Kotlin 的路径。
- 全部修复均已提交并推送到 `origin/main`。

## 已知残余风险

- 未连接真机执行 GPU/JNI 压力测试与 Android instrumentation 测试；设备特定纹理上限已按运行时 `limits()` 兜底，但真机路径仍需在有设备环境复验。
