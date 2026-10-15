# fyne-io/terminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/fyne-terminal`
- 技术栈：Go + Fyne graphics toolkit; self-developed terminal escape parser (Go)
- 定位：Fyne terminal emulator widget
- 许可证：MIT/BSD? LICENSE
- 主要模块：apc/dcs/escape/mouse/osc/output/select/render, tests many

## 全仓库结构与关键代码
- `term.go`, `escape.go`, `dcs.go`, `apc.go`, `osc.go`, `output.go`, `input.go`, `mouse.go`, `select.go`, `render.go`, `test_data`.
- Cross-platform PTY handling: darwin/unix/windows.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Go/Fyne not Android Rust; VT self-dev.
- Test data useful.

## 可直接使用的库/工具
- Go module not for Android Rust.
- Fyne not Android.

## 可间接借鉴
- Terminal parser structure (CSI/OSC/DCS) as design lesson.
- test_data directory may provide VT expected behavior.
- Go PTY cross-platform integration.

## 可稍微修改使用的代码/设计
- Go code not portable.

## 测试数据/测试用例
- `*_test.go` and test_data; valuable for behavior comparison.

## 设计模式
- Terminal state object with escape processors and render update.

## 其他结论
- MIT-ish; not direct. C-grade Go UI.
- Good if needed Go reference only.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
