# wilsonglasser/oryxis 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/oryxis`
- 技术栈：Rust + iced (wgpu) + alacritty_terminal (VT core) + native vault/SSH/FIDO2
- 定位：本地加密 vault 的 SSH/SFTP/serial/telnet/mosh 桌面客户端，Termius 替代
- 许可证：AGPL-3.0
- UI/GPU：iced with wgpu.
- VT core: alacritty_terminal, not Ghostty.

## 全仓库结构与关键代码
- `crates/oryxis-terminal` is a terminal wrapper; renderer coupling is not determined in this report.
- `oryxis-app`: UI.
- `oryxis-ssh`, `oryxis-mosh` (screen uses alacritty Terminal types), `oryxis-vault`, FIDO2, sync/cloud.
- configs native build.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Oryxis desktop SSH client, VT core alacritty; no Android.
- wgpu rendering similarities; iced GUI not Android.
- VT core alternative conflicts single-source.

## 可直接使用的库/工具
- Rust crates include ssh/vault/zmodem utility features and are not terminal rendering dependencies.

## 可间接借鉴
- PTY/SSH screen integration with alacritty term.
- iced/wgpu widget terminal rendering.
- Config vault patterns (if mobile app later, but privacy concerns).

## 可稍微修改使用的代码/设计
- Terminal widget code be ported but iced/wgpu desktop vs Android Compose no.

## 测试数据/测试用例
- crate tests were counted but their assertion scope was not expanded.
- alacritty VT test suite not applicable directly.

## 设计模式
- App + core + terminal separation.
- VT terminal core behind app widget.

## 其他结论
- Desktop SSH; not direct, Rust/wgpu patterns useful. VT single-source conflict.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
