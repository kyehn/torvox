# KDE/konsole 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/konsole`
- 技术栈：C++ (Qt) + CMake, GNU/Linux/FreeBSD/macOS/Windows targets; self-developed VT (Vt102Emulation)
- 定位：KDE terminal emulator, profiles/sessions, tabs, search, ZMODEM, Kitty keys
- 许可证：GPL v2 (COPYING) + docs
- 主要模块：Emulation, Screen, ScreenWindow, terminalDisplay, session, pty, profile, history, keyboardtranslator, decorators.

## 全仓库结构与关键代码
- `src/Vt102Emulation.cpp`：VT parser/interpreter based on vt100.net diagram, handles APC (kitty), colors, modes.
- `src/Screen.cpp/h`：terminal screen/buffer.
- `src/terminalDisplay/*`：Qt rendering/paint.
- `src/Pty.cpp`, `session/*`, `profile/*`: PTY and profiles.
- `tests/`, `autotests/`：regression tests.
- `characters`, `colorscheme`, `data`, `tools`.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Konsole Qt/C++ desktop, VT self-developed; conflicts with single-source and Android.
- Some interaction features (copy/paste, key bindings, history, search) can be referenced.

## 可直接使用的库/工具
- 无. Qt for Android possible but heavy and mismatched.

## 可间接借鉴
- VT parser design based on vt100.net (only educational).
- Kitty keyboard protocol support.
- Profile/session management UX.
- Screen buffer/window tracking.

## 可稍微修改使用的代码/设计
- C++ VT can't be embedded into Rust without significant adaptation; license GPL.
- Config/profile interaction design is documented and can only be reproduced by TorVox in Kotlin/Compose; it is not imported from Konsole.

## 测试数据/测试用例
- autotests (`tests/`, `autotests/`) Cover emulation/screen. compare with vttest but expected values need external truth audit.

## 设计模式
- MVC-ish: Emulation + Screen + Display + ViewManager.
- plugin system, session model.

## 其他结论
- GPL v2; direct reuse licensing concerns.
- Desktop QTerminal emulation, C-grade reference (desktop) prior persistent.

## 四次审阅记录
- 准确性: based on files.
- 完整性.
- 对比正确性.
- 文档一致性.
