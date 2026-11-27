# mtty-term 深度审查

- 来源：https://github.com/spizganed/mtty-term
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：mtty 的终端核心子模块，Android View + libghostty-vt JNI + 键编码，刻意不含 app 代码，可在 JVM 上测试。

### 分层与关键机制

- `TermCore.kt` 定义核心契约，`TerminalHost.kt` 定义回报给 app 的事件（reply、OSC 52、标题、bell）；`GhosttyTerminal.kt` 是唯一实现，`GhosttyNative.kt` 调 JNI shim（位于母仓 `tools/libghostty/shim.c`）。
- JNI shim 为每个 handle 持有一个 libghostty terminal；行在视图请求时读取（`nReadRows`，每单元 24 字节），不是每次 feed 后读。原生库缺失时 `GhosttyTerminal.available` 为 false，相关 JVM 测试跳过。
- `Row.kt`/`Style.kt`：一个单元是一个码点加一个打包 `Long` 的样式；宽字符占两单元，组合标记进 `extra`。历史由 libghostty 保存，`GhosttyTerminal` 只复制视图可见行。文档称 60000 行洪水时约 3 ms/帧。
- `Glyphs.kt`/`BoxTable.kt`：框线、块元素、Powerline 分隔符用形状绘制，避免手机字体缺字和单元间缝隙；表由母仓 `tools/gen-box.py` 从 Unicode 数据生成。
- `Keys.kt`：键、文本、鼠标按核心当前模式编码为字节，包括 Kitty keyboard 与鼠标 SGR；`Find.kt` 逐行大小写不敏感搜索，`OneLine.kt` 复制软换行为一行但保留 `!`/`$ ` 开头的硬断。
- `TermView.kt` 是单个大 View，处理触摸、键、IME、选择、缩放与同步输出（DEC mode 2026）。
- 5 个测试文件；README 写明完整母仓下 `:term:testDebugUnitTest` 有 95 个测试，核心测试需先构建 host shim。已知差异：libghostty 只保存 OSC 8 URI，同 URI 不同 id 会共享 link id。

### 验证与风险

- 该仓库不能独立构建，依赖母仓的 JNI shim 与生成脚本；单独 clone 只能读源码，不能复现测试。
- 单 View 承担过多职责，长期维护成本高；但“核心可替换、模块不依赖 app”的边界使 JVM 测试可行。
- 单元 24 字节的 native 行协议是性能关键，变更后必须同步 shim 与 Java 读取端。

### 本地取舍

这是本地 Ghostty 路线的最近参照。可核对三点：本地 `GhosttyNative` 读取行时是否也只在需要时复制；`TerminalInputEncoder` 的 Kitty 标志是否覆盖 mtty `Keys.kt` 列出的模式；框线/块字符是否需要形状兜底。OSC 8 id 合并差异应在本地做兼容说明或测试，而不是假设上游行为一致。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/terminal/ghostty_terminal/public_api.rs` | 对照 Ghostty 句柄与 sanitize 边界 |
| `native/src/terminal/ghostty_terminal/internal/input_filter.rs` | 对照 Kitty 探测与 CSI 解析饱和 |
| `native/src/terminal/ghostty_terminal/key_encoding_tests.rs` | 对照键编码事实与回归用例 |
| `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt` | 对照 Kitty 标志覆盖矩阵 |
| `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | 对照按需读行与批量提交 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.gitignore` | TEXT | 16 | 2 | `d7bfa9d1404f` |
| `LICENSE` | DECLARED_EXCLUSION | 11358 | 202 | `cfc7749b96f6` |
| `NOTICE` | TEXT | 547 | 9 | `b27c0ed26724` |
| `README.md` | TEXT | 1022 | 22 | `42e37ec21a69` |
| `build.gradle.kts` | TEXT | 803 | 33 | `a4da6cfed429` |
| `docs/ARCHITECTURE.md` | TEXT | 3797 | 45 | `e59e9c74badf` |
| `src/main/kotlin/com/spizganed/mtty/term/BoxTable.kt` | TEXT | 9519 | 173 | `769043f45412` |
| `src/main/kotlin/com/spizganed/mtty/term/Find.kt` | TEXT | 1774 | 41 | `779cca83e57b` |
| `src/main/kotlin/com/spizganed/mtty/term/GhosttyNative.kt` | TEXT | 2142 | 47 | `229016bb20ad` |
| `src/main/kotlin/com/spizganed/mtty/term/GhosttyTerminal.kt` | TEXT | 9094 | 197 | `56dc099b09a1` |
| `src/main/kotlin/com/spizganed/mtty/term/Glyphs.kt` | TEXT | 8379 | 176 | `19cc3373f9b3` |
| `src/main/kotlin/com/spizganed/mtty/term/Keys.kt` | TEXT | 11929 | 262 | `da7d548bf4e2` |
| `src/main/kotlin/com/spizganed/mtty/term/OneLine.kt` | TEXT | 1315 | 27 | `6c4c43861f6b` |
| `src/main/kotlin/com/spizganed/mtty/term/Row.kt` | TEXT | 3883 | 107 | `7314cb2870b5` |
| `src/main/kotlin/com/spizganed/mtty/term/Style.kt` | TEXT | 4598 | 111 | `43e45fcd3af8` |
| `src/main/kotlin/com/spizganed/mtty/term/TermCore.kt` | TEXT | 3639 | 100 | `80991f30111d` |
| `src/main/kotlin/com/spizganed/mtty/term/TermView.kt` | TEXT | 51793 | 1157 | `af40d4751f1c` |
| `src/test/kotlin/com/spizganed/mtty/term/CoreTest.kt` | TEXT | 17152 | 512 | `ef78676da606` |
| `src/test/kotlin/com/spizganed/mtty/term/GhosttyTerminalTest.kt` | TEXT | 510 | 11 | `7e1f0de380ca` |
| `src/test/kotlin/com/spizganed/mtty/term/KeysTest.kt` | TEXT | 6419 | 155 | `e8b22f385944` |
| `src/test/kotlin/com/spizganed/mtty/term/OneLineTest.kt` | TEXT | 909 | 23 | `95aeef4770aa` |
| `src/test/kotlin/com/spizganed/mtty/term/StressTest.kt` | TEXT | 3812 | 76 | `5c56300d52b1` |

## 复算命令

```sh
git clone --depth 1 https://github.com/spizganed/mtty-term /tmp/opencode/external/mtty-term
git -C /tmp/opencode/external/mtty-term ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/mtty-term ls-files | wc -l
```
