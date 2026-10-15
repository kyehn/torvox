# lihop/godot-xterm 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/godot-xterm`
- 技术栈：Godot 4.3+ GDExtension + C++17 + `libtsm`、`libuv`、`node-pty`；Godot 渲染/Shader/C++。
- 定位：Godot 场景与编辑器中的交互式终端控件，支持 Linux/macOS/Windows，部分 Web (HTML5)。
- 许可证：MIT。
- 结构：`addons/godot_xterm/native/src`（Terminal/PTY/shader/render）、大量 GDS resources/shaders/ editor integration、`tests/`、CI。

## 代码 reading 摘要
- `addons/godot_xterm/native/src/terminal.h`：terminal 是 `godot::Control`，封装 libtsm screen + VTE；维护 cols/rows/max_scrollback/bell/blinking、font size、palette、rendering canvas/image/shader、selection/copy、mouse、keyboard。
- PTY 通过 libuv/node-pty 适配（`pty_unix.cpp`, `pty_win.cpp`, `pty_unix.h`），负责 spawn/resize/read。
- Rendering 通过 `initialize_rendering`, `_draw_cb`, style box/shaders（`shaders/*.gdshader`），Godot rendering server resources。
- 输入 `_gui_input` -> `_handle_key_input`, `_handle_mouse_wheel`, `_handle_selection`。
- Assets/utilities: tput, themes, fonts (JetBrains Mono), xrdb theme import plugin, asciicast import support.
- Tests: GDScript integration/unit + smoke Web + test scenes.

## 可直接使用的库/工具
- libtsm（ terminal emulator core + rendering data model）对 Kotlin/Rust 不直接可链入 Android JNI 时可作为 C backend，但本项目已经 Ghostty VT (`libghostty-vt`)；
- GodotXterm 作为整体是 Godot editor/game 集成，与 Android JNI/compose 不相关；
- libuv/node-pty 可单独提供 cross-platform PTY，对 Rust/NDK 有间接价值，但 crates `portable-pty` 更符合 Rust。

## 可间接利用
- `libtsm` screen attributes 和 `_draw_cb` callback 设计；但内部渲染坐标映射可参考；
- ConPTY resize 重试/conpty quirks（`CONPTY_RESIZE_STABLE_FRAMES`, `CONPTY_RESIZE_MAX_FRAMES`)；Android Linux PTY 需 ECHO/resize，但 ConPTY 不适用；
- 字体 metric/cell size, theme palettes, inverse-mode behavior, bell cooldown;
- XRDB/color theme parsing and themes packaging.

## 可稍微修改使用的代码/设计
- `Terminal` 的 public API 设计（cols/rows/cell_size/max_scrollback/getters/setters）可照作 Compose `TerminalView` contract;
- selection/copy API (`copy_all`, `copy_selection`, `select`, copy-on-selection) 可映射 ViewModel state;
- key/shell bridge design;
- shader rendering concepts not directly wgpu（Godot shaders），但 signal age and cell attribute image pattern can inspire cell-array batch upload.

## 测试数据/测试用例
- `test/scenes`/`.gd*` integration tests (Godot test framework)
- `test/import_plugins/xrdb` tests
- asciicast samples (gz/counter), tput utils
- shell test scripts (`colortest.sh`)
- not standard text VT golden suite; useful for rendering/resize behavior manually.
- Resize/conpty/bell scenarios exist in repository test paths; their executable commands were not run here.

## 设计模式
- Native Control/God extension model; PTY via plugin service + Godot rendering;
- class split Terminal vs PTY; callback-based async terminal and ConPTY resize coordination;
- resource set for themes/fonts; editor integration plugins.
- Input handling and selection nested separately.
- Rendering dirty flags/age checks.

## 对比 torvox
- Rendering: godot-xterm uses Godot's rendering + GLSL shaders; torvox uses wgpu + Kotlin `RenderDriver`/TextureBrush entries and custom glyph atlasing. Not reusable.
- VT: godot-xterm uses libtsm; torvox uses Ghostty VT, stricter/modern terminal semantics. Configures capabilities/perf; adding smunalibtsm would violate Ghostty-only architecture.
- UI: Godot node tree versus Compose; no direct mapping.
- PTY: libuv/node-pty versus Rust `portable-pty` + Android JNI; porting native PTY complex, unsupported on Android.
- Architecture: Godot game engine and editor-centric versus Android app. Not target runtime.
- Useful only as reference for terminal control API, themes, PTY quirks.

## 结论
- 直接库可取值：libtsm only if C dependency accepted; but torvox mandates libghostty-vt, so not.
- 间接可借鉴：ConPTY handling, selection/shell contract, font/attribute packing; pure reference.
- 不应导入成 Android module; Godot dependencies too large.

## 四次审阅记录
- 准确性：校验 README/terminal.h/structure（libtsm/libuv/node-pty，Godot, MIT，tests）。
- 完整性：覆盖 native VT/render/input/PTY/tests/docs; excludes deep code but enough.
- 对比正确性：强调 Ghostty vs libtsm and Godot runtime incompatibility.
- 文档一致性：slug and filename match repository path.
