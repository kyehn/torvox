# plyra-terminal 上游审查

- 上游：https://github.com/J-x-Z/plyra-terminal @ `7bdc3a3`（2026-11-18）。
- 定位：跨桌面与 Android 的 GPU 终端，核心卖点是特权隔离、可验证构建与严格的屏幕协议；渲染层使用 glyphon 0.8。

## 架构与数据流

- `terminal-core`：共享屏幕、有界历史、选择与经过校验的屏幕事件协议；`session-manager` 拥有 PTY、可执行参数、输入队列、resize、鼠标与焦点协议。
- `terminal-render/src/lib.rs`：`TerminalRenderer` 持有 `glyphon::TextRenderer`、`Cache`、两个 `TextAtlas` 与两个 `TextRenderer`（主文本 + preedit）；每次 `prepare` 调用 `shape_cache.begin_frame()`，把运行段和选择切成 `PlacedText`，`encode` 时按背景→文本→装饰/光标→preedit 的顺序提交。
- 选择实现是把每行按选中像素范围切成最多三个 TextArea，用不同 `default_color` 重复绘制同一 buffer；背景另画矩形。`parse_line` 校验列数、运行宽度和与、行号范围，越界即整帧拒绝。
- `shaped_runs.rs` 以文本、列宽、粗斜、字号和行高为 key 缓存 Buffer，逐帧淘汰未命中项；`set_font_family` 会清空缓存。缓存只保存布局，颜色由 TextArea 提供，因此可跨颜色复用。
- macOS helper 持有真实 PTY 与提取出的 libvterm 0.3.3，通过有界 JSON 屏幕/滚动事件送到 Rust 读线程；`PROVENANCE.json` 记录 vendor patch 与源码指纹。
- `privilege-policy`/`privilege-broker` 做能力绑定、一次性票据、执行前再验证、撤销与审计；Android 路径文档标为 smoke/lifecycle 验证，桌面 active target 是 Apple Silicon。

## 已核实边界

- 屏幕事件先校验再进渲染，把不可信输出挡在模型外；但 vendored libvterm、特权 broker、插件与 MCP 各自扩大信任面，不能只按测试文件数量判断成熟度。
- 选择按像素切 TextArea：跨越切分边界的宽字形可能被两个区域各画一次，颜色覆盖顺序依赖 glyphon 的 clip 行为；本地若不采用 glyphon 无需承担此问题。
- 两个 `TextRenderer` 共享同一 `TextAtlas`，提交顺序和 `trim()` 时机是渲染正确性的隐含前提；本地自有管线没有这层耦合。

## 对 torvox 的取舍

- 可借鉴“屏幕事件校验后再进渲染”和“空闲不请求帧”；本地 `native/src/terminal/output_processor.rs` 已经是 OSC/CSI 分派边界，不引入第二套屏幕协议。
- `shaped_runs.rs` 的缓存 key 思路可与 `native/src/render/font/mod.rs` 的精确光栅参数对照；glyphon 的 Buffer 缓存不迁移。
- 特权代理、插件沙箱和 MCP 不采纳；本地仓库明确不考虑安全/隐私特性之外的特权模型。
- 跳过：未运行桌面/Android 构建与 GPU 基准（需 Apple Silicon 或 Android NDK/GPU）；风险是 glyphon 0.8 与本地 cosmic-text 管线的差异未实测。
