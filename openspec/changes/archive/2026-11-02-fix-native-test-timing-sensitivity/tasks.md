# 任务

## 一、缺陷核定

- [x] 确认红的是**用例**而非产品代码：同一份代码在 `LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`
      下稳定全绿（787 passed / 19.76s），在默认 Debug 上游构建下偶发红 4~5 项。
- [x] 定位根因一（逐行写放大开销）：命令批尾是唯一的帧发布点，每满 64 条命令
      重建一次整幅网格；`scrollback_deep_buffer_does_not_collapse` 逐行写 2.5 万行
      即触发数百次整幅重建，与被测的回滚语义无关。
- [x] 定位根因二（上游构建模式放大成本）：`libghostty-vt-sys` 的 `zig_optimize_mode()`
      在 cargo dev profile 下取 `Debug`，Debug 下 `Screen.cursorDownScroll` /
      `PageList.grow` 每次滚动都跑 `assertIntegrity()`（全量遍历页面与 tracked pins）。
- [x] 确认整段写与逐行写对上游**等价**：上游解析器跨调用保持状态，
      分片写入与整段写入产生同一条字节流的同一解析结果。
- [x] 确认放宽期限不削弱判别力：被测的失败模式是「批无界 → 通道恒非空 → 排空永不完成」，
      该失败在任何期限下都不会排空；期限只区分「VT 线程卡死」与「机器慢」。

## 二、实现

- [x] `ghostty_terminal::tests/mod.rs`：新增 `write_lines` 辅助（整段行文本一次 `vt_write`）
      与 `DRAIN_TIMEOUT`（30s）常量，并写明各自的判别力边界。
- [x] `selection_search.rs`：
      - `scrollback_cap_is_honored` 改整段写 + `flush_with_timeout(DRAIN_TIMEOUT)`。
      - `scrollback_keeps_order_and_newest_line_stays_visible` 同上。
      - `scrollback_deep_buffer_does_not_collapse` 批次 500 → 2 500 行，每批整段写 +
        `flush_with_timeout(DRAIN_TIMEOUT)`。
- [x] `grid_text.rs`：`read_all_text_answers_repeated_queries_within_query_timeout`
      改整段写 + 排空断言（排空未完成时断言消息直指「回滚只反映已处理的部分」）。
- [x] `batch_latency.rs`：命令积压排空期限 10s → 60s；`flush_with_timeout` 5s → 30s。

## 三、验证

- [x] 默认 Debug 上游构建（cargo dev profile）全量套件：`787 passed; 0 failed`（69.36s）。
- [x] `LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast` 全量套件：`787 passed; 0 failed`（19.76s）。
- [x] 并发不变量测试（独立目标 `native/tests/concurrency.rs`）：`2 passed; 0 failed`。
- [x] `cargo clippy --all-targets -- -D warnings`：0 告警。
- [x] `cargo fmt --check`：无差异。
- [x] 移除临时探针用例 `tmp_probe_write_cost`（仅用于测量写入成本，无判别力）。

## 四、回归钉的判别力核对

- [x] 逐个压力用例核对：放宽期限后，被测的失败模式（批无界 / 回滚不增长 /
      查询无应答）仍 MUST 判红——期限只排除「机器慢」这一个非缺陷原因。
