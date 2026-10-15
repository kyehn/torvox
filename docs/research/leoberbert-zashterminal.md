# leoberbert/zashterminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/zashterminal`
- 技术栈：Python + GTK4/VTE? Vte widgets, session management, SSC/SFTP, AI, syntax highlight.
- 定位： Feature-rich Linux desktop terminal, GTK4/Adwaita.
- 许可证：GPL-3.0
- 主要模块：terminal manager/spawner/tabs/highlighter, sessions, ui, settings, utils.

## 全仓库结构与关键代码
- `terminal/`: manager.py, spawner.py, tabs.py, ai_assistant.py, highlighter.
- `sessions/`: tree, storage.
- `settings`: GTK widgets.
- All python scripts; VTE binds via python VTE bindings.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Linux VTE/GTK app; no Android.
- ZashTerminal features such as SSH sessions and manager UI can inform product UX ideas without importing implementation.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- Session management and SecureCRT import design.
- File manager side panel pattern.
- Input broadcasting, quick prompts.
- AI assistant privacy model (explicit send).

## 可稍微修改使用的代码/设计
- Python desktop source not Android.

## 测试数据/测试用例
- Tests are present in repository form; coverage assertions not enumerated in this report.
- Not useful VT.

## 设计模式
- GTK application with session model and VTE.

## 其他结论
- Desktop C-grade (Gnome). Not direct.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
