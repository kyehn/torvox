# 任务

## 准备

- [x] 导入 ghostty `parser-initial` `stream-initial` `osc-initial` 到 `native/src/terminal/testdata/seeds/`，去除各文件首字节，附带来源与许可说明
- [x] 导入 31 份一致的 alacritty 录音到 `native/src/terminal/testdata/conformance/`，附带来源、许可与 14 份差异逐份记录
- [x] 新增 `.gitattributes`，语料目录整体标记为不做换行规范化

## 语料运行器

- [x] `TestSnapshot` 增加 `scrollback` 字段，`SNAPSHOT_VERSION` 递增，存量 10 份期望文件重生成
- [x] 语料运行器改为按期望文件的 `rows`/`cols`/`scrollback` 创建终端，移除 `CORPUS_ROWS`/`CORPUS_COLS`/`CORPUS_SCROLLBACK` 常量
- [x] 语料扫描拆分为「按目录装载」与「单条执行」两个函数，供跨引擎语料复用，差异报告标明语料名

## 新测试

- [x] 新增种子分块写入不变性测试：整块写入与逐字节写入（逐块确认排空）结果一致，语料为空判定失败
- [x] 新增跨引擎一致性测试：按期望文件创建终端，断言屏幕文本、光标与尺寸，不断言样式与回滚
- [x] 新增宽度分类一致性测试：覆盖 CJK 表意/扩展/音节/兼容/彝文区段，与 `unicode-width` 一致；`native/Cargo.toml` 开发依赖新增 `unicode-width`

## 验证

- [x] `cargo test` 全量通过，`cargo clippy` 与 `cargo fmt --check` 无告警
- [x] 语义抖动自检：临时改动 `.seq` 输入、删除期望文件、改动期望文本各一次，确认对应测试失败
- [x] 确认 `native/src/terminal/testdata/` 下的 `.seq` 与 `.json` 仍严格成对
- [ ] 更新 `openspec/specs/terminal-state-regression/spec.md` 与新增 `openspec/specs/upstream-vt-test-assets/spec.md`
- [ ] 归档本 change
