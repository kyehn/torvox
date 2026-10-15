# GlobalTechInfo/RedTerm 研究报告

## 仓库概况
- 兯隆路径：`/tmp/opencode/redterm`
- 技术栈：Kotlin Android app, depends Termux terminal-emulator:terminal-view (v0.118.3), proot natives, JNI.
- 定位：Android Linux distro runner via proot; includes terminal UI, SSH/SFTP, distro backups.
- 许可证：GPL-3.0

## 全仓库结构与关键代码
- `:app` Gradle; termux emulator view dependency.
- Native proot build script `native/build-proot.sh`.
- SSH binary includes, sessions, file manager.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- RedTerm exercises Android proot (not VT custom) and Termux native libs.
- Distribution model can inform Android packaging and multi-session UI.

## 可直接使用的库/工具
- Termux `terminal-emulator` library is LGPL? actual license MIT/Apache? (Termux terminal-emulator is Apache 2.0). It provides most Android terminal VT standard.
- RedTerm is not an Android VT library candidate; TorVox uses Ghostty as sole VT source.
- Reusing libproot.so directly conflicts with Torvox not embedding proot; it is out of scope.
- Native PTY/proot architectures valuable.

## 可间接借鉴
- proot root-less Linux environment integration.
- Termux terminal-view UI & input.
- Multi-session switcher, themes, SSH keyring.
- Foreground notification session.
- JNI prebuilt align 16KB.

## 可稍微修改使用的代码/设计
- Termux terminal emulator comes as library; can be checked as fallback.
- Native `libproot` integration requires distribution license compliance.
- UI layering concept is a design reference only; importing Userland UI dependencies is not planned for TorVox.
- Good to keep as reference for Android proot and packages.

## 测试数据/测试用例
- Tests under app/src/test.
- Termux library has built-in tests; external.

## 设计模式
- Android native JNI + Kotlin UI.
- Service foreground + multi session.

## 其他结论
- Reference Android terminal deployment, not VT correctness.
- License GPL, may not match.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
