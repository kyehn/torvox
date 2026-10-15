# missdeer/mostty 研究报告

## 仓库概况
- 克隆路径：`/tmp/mostty`
- 技术栈：Zig 0.16 + libghostty VT（ghostty 1.3.2-dev 依赖）+ Direct3D 11/12、OpenGL 4.6、Vulkan 等本地渲染器，Zig 构建系统
- 定位：Windows 10/11 与 macOS 13+ 原生终端，基于 Ghostty VT core，libghostty sprite vendor
- 许可证：仓库根未见 LICENSE（ vendors/z2d、ghostty-sprite 包含各自许可证）
- 主要模块：`build.zig` / `build.zig.zon`（依赖 ghostty/z2d）、`src/`（Win32/macOS PTY、渲染器、تعاملات ring）、`ARCHITECTURE.md`（异步字节流设计）

## 对比本项目
- 相似点：都以 libghostty VT 为唯一 VT core，强调 Rust/Zig 原生性能；mostty 对 PTY→ UI 线程交接有清晰工程设计。
- 差异：mostty 是桌面/Win32+macOS Zig 原生应用， Torvox 是 Android Kotlin + Compose + wgpu/Vulkan；渲染后端和 UI 异质。
- Android 可学：byte ring + 异步 drain、定时器续排、按 session/pane 规模 bounded quantum，可映射为 Kotlin/Rust JPNI producer-consumer 与 frame/backpressure。

## 可直接使用的库/工具
- `libghostty` Zig 依赖（本仓已通过 JNI attach，不直接 substitutable）
- Ghostty-compatible themes/sprite vendor 机制可借鉴
- ConPTY 与 Windows 鼠标/IMER 为平台细节，不适用于 Android。

## 可间接借鉴
- `src/win32/pty_ring.zig` SPSC 字节环 + `PostMessage` hand-off pattern.
- Edge-triggered notification (`posted` bool) 限制 wake-up 链。
- Reader ring full 时 parking on wake event; drain 时 signaling.
- Pane close sets `reader_stop`、signal wake.
- Bounded 8 ms deadline 与按Pane 2 ms quantum.
- Tabs、session closure behavior、submenu launcher pattern documentation.

## 可稍微修改使用的代码/设计
- Ring buffer 实现需要 Rust/JNI 平等；可移植为数组+atomics，但需适配 WebView? N/A.
- Mostty theme loading and config reload are settings design references, not code intended for adoption.
- Ghostty VT interactions (title/PWD/Hyperlink queries) align.

## 测试数据/测试用例
- `tools/` PowerShell scripts and probes; not a formal VT test corpus.
- ConPTY image policy and renderer acceptance tests.
- borrow CLI smoke probes rather than VT fixtures.

## 设计模式
- UI 线程与 PTY reader 线程通过 bounded SPSC byte ring + notify.
- Declarative pane registry by id, `reader_stop` lifecycle.
- Renderer backend selection and startup failure diagnostics.
- Configuration-driven Ghostty theme compatibility.

## 其他结论
- 可直接 conclusions: per-pane bounded quantum 解决 Android 长 SSH 输出 starvation; wake-event backpressure useful.
- Do not adopt mostty's entire native renderer as it conflicts with Compose + wgpu.
- mostty shows Rust not required for high-performance terminal if Zig + Ghostty.
- No Android target; thus only engineering patterns from concurrency planning.

## 四次审阅记录
- 审阅 README/ARCH 中具体 PTY ring 结构，并核对本仓已有 JNI/Wgpu handoff 路径。
- 验证报告中声明的 Zepto dependencies from build.zig.zon.
- 对比 wording 无过誉 Android relevance.
- 确认 license 未声明、因此不 asserting.
