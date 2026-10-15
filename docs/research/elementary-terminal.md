# elementary/terminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/elementary-terminal`
- 技术栈：Vala + GTK4 + Granite + vte-2.91-gtk4
- 定位：elementary OS (Pantheon) terminal emulator
- 许可证：GPL (COPYING)
- 主要模块：Application, MainWindow, TerminalWidget/View, Settings, Search, unsafe paste dialog, Zoom overlay.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Elementary terminal depends on libvte, another legacy VT core; UI GTK/Vala.
- Not Android.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- VTE integration shell settings.
- Unsafe paste dialog UX.
- Search toolbar design.
- Elementary/GNOME theme conventions.

## 可稍微修改使用的代码/设计
- Vala code not portable; VTE dependency GPL and incompatible VT philosophy.

## 测试数据/测试用例
- src/tests: Application, UnsafePasteTest only.
- Not useful.

## 设计模式
- GNOME-style GApplication + settings schema.
- VTE widget wrapper.

## 其他结论
- C-grade Linux desktop reference. Prior ecosystem rates VTE-based Linux Gnome family C.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
