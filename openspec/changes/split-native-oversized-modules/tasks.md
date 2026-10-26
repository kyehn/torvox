# Tasks

## 1. 超长函数

- [ ] 1.1 `terminal/ghostty_terminal/internal.rs`：`run_inner`（429 行）拆成
      「状态初始化 / 等待事件 / 空闲推进 / 命令批处理」四段，命令分派再按命令族
      拆成独立函数，
      验证：`cargo test --lib` 全绿，aislop 的「函数过长」告警消失

## 2. JNI 层

- [ ] 2.1 `android/ffi.rs`：共享全局状态与渲染状态类型迁到 `ffi/state.rs`
- [ ] 2.2 `android/ffi.rs`：会话生命周期入口迁到 `ffi/session.rs`
- [ ] 2.3 `android/ffi.rs`：渲染帧入口迁到 `ffi/render.rs`
- [ ] 2.4 `android/ffi.rs`：键鼠与粘贴编码迁到 `ffi/input.rs`
- [ ] 2.5 `android/ffi.rs`：文本/选区/搜索查询迁到 `ffi/query.rs`
- [ ] 2.6 `android/ffi.rs`：字体、字号与栅格缩放迁到 `ffi/font.rs`
- [ ] 2.7 `android/ffi.rs`：剪贴板请求登记迁到 `ffi/clipboard.rs`
- [ ] 2.8 `android/ffi.rs`：事件泵迁到 `ffi/events.rs`
- [ ] 2.9 `android/ffi.rs`：内联测试模块迁到 `ffi/tests/` 下的独立文件

## 3. 渲染层

- [ ] 3.1 `render/font/mod.rs` 按职责拆分
- [ ] 3.2 `render/font/font_db.rs` 按职责拆分
- [ ] 3.3 `render/cell_builder.rs` 按职责拆分
- [ ] 3.4 `render/context.rs` 按职责拆分
- [ ] 3.5 `render/pass.rs` 按职责拆分
- [ ] 3.6 `render/tests.rs` 按主题拆分

## 4. 终端层

- [ ] 4.1 `terminal/session.rs` 按职责拆分
- [ ] 4.2 `terminal/pty.rs` 按职责拆分
- [ ] 4.3 `terminal/ghostty_terminal/internal.rs` 除 `run_inner` 外按职责拆分
- [ ] 4.4 `terminal/ghostty_terminal/tests.rs` 按主题拆分

## 5. 集成验证

- [ ] 5.1 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [ ] 5.2 `npx --yes aislop@latest scan` 的 Code Quality 告警归零
- [ ] 5.3 `openspec validate --all --strict` 全绿

## Workflow follow-up

- 归档变更。
