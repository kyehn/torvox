# mtty-term 上游审查

- 上游：https://github.com/spizganed/mtty-term @ `5b079a9`（2026-11-25）；终端核心子模块，不含 app 代码。
- 定位：Android View + libghostty-vt JNI + 键编码，可在 JVM 上测试；与本地 Ghostty 路线最近。

## 架构与数据流

- `TermCore.kt` 定义核心契约，`TerminalHost.kt` 定义回报给 app 的事件（reply、OSC 52、标题、bell）；`GhosttyTerminal.kt` 是唯一实现，`GhosttyNative.kt` 调 JNI shim（位于母仓 `tools/libghostty/shim.c`）。
- shim 为每个 handle 持一个 libghostty terminal；行在视图请求时由 `nReadRows` 复制（每单元 24 字节），不是每次 feed 后读。原生库缺失时 `GhosttyTerminal.available` 为 false，JVM 测试跳过。
- `Row.kt`/`Style.kt`：一个单元一个码点加一个打包 `Long` 样式；宽字符占两单元，组合标记进 `extra`。历史由 libghostty 保存，Java 侧只复制可见行。
- `Glyphs.kt`/`BoxTable.kt`：框线、块元素、Powerline 分隔符用形状绘制，绕开手机字体缺字和单元缝隙；表由母仓 `tools/gen-box.py` 生成。
- `Keys.kt` 按核心当前模式编码键、文本与鼠标（含 Kitty keyboard、鼠标 SGR）；`Find.kt` 逐行大小写不敏感搜索，`OneLine.kt` 合并软换行为一行但保留硬断。
- `TermView.kt` 单 View 处理触摸、键、IME、选择、缩放与同步输出（DEC mode 2026）。

## 已核实边界

- 该仓库不能独立构建或跑测试：JNI shim 与生成脚本在母仓，单 clone 只能读源码；README 给出的 95 个测试需先构建 host shim。
- libghostty 只保存 OSC 8 URI，同 URI 不同 id 会共享 link id，需做兼容说明或测试，不能假设与上游一致。
- 24 字节行协议是性能关键，变更必须同步 shim 与 Java 读取端；单 View 职责过多是长期维护成本。

## 对 torvox 的取舍

- 对照 `native/src/terminal/ghostty_terminal/public_api.rs`：确认本地也是按需读行，而非每次 feed 复制全屏。
- 对照 `native/src/terminal/ghostty_terminal/key_encoding_tests.rs` 与 `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt`：逐项核对 Kitty 标志覆盖，不要只看“已支持 Kitty”。
- `BoxTable` 的形状兜底可借鉴思路；本地字体走 `native/src/render/font/mod.rs`，不直接复制表。
- 跳过：未构建母仓 shim 与 JNI（需母仓与 native 工具链）；风险是 95 个测试未在本会话复现。
