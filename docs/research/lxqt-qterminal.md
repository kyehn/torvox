# lxqt/qterminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/qterminal`
- 技术栈：C++ + Qt6 + qtermwidget (QTermWidget), LXQt 集成, CMake
- 定位：轻量 Qt 终端模拟器，特性包括 tabs, bookmarks, drop-down mode, DBus control
- 许可证：GPL v2 (LICENSE)
- 主要模块：mainwindow/tab/termwidget holder/properties, bookmarks, translations
- VT emulation delegated to qtermwidget (Konsole-derived).

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- qterminal is Qt UI thin wrapper around qtermwidget; architecture desktop/QML-less.
- VT emulation is not libghostty; self-dev? qtermwidget legacy.
- No Android rendering path.

## 可直接使用的库/工具
- 无.

## 可间接借鉴
- Tab/session management and profiles UX.
- Bookmark/drop-down terminal pattern (not applicable to single Android terminal UI).
- Language/i18n organizational patterns.

## 可稍微修改使用的代码/设计
- C++/Qt code not portable; no.

## 测试数据/测试用例
- test directory but qterminal mostly GUI; limited VT expected data.

## 设计模式
- Model-View style with terminal widget holder.
- DBus-based remote control.

## 其他结论
- Desktop D-grade reference, not applicable; associated qtermwidget has same code lineage as Konsole (GPL).

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档一致性.
