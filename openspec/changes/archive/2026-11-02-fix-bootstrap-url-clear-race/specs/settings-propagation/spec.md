## ADDED Requirements

### Requirement: 清除应用数据作废清除前的待写编辑

清除应用数据 MUST 作废**清除之前**编辑的引导 URL：那一版的待写值 MUST NOT 在清除
之后落盘，MUST NOT 重建已被删除的 `preferences_pb`，MUST NOT 让「已清除」的提示变成
假话。

作废 MUST NOT 依赖「停用写入的窗口恰好覆盖待写值的到期时刻」：清除要删目录并重建
prefs，耗时与防抖窗口（300ms）互不相干，故清除之前的编辑 MUST 由**清除轮次**作废，
无论它何时到期。清除之后的新编辑 MUST 照常落盘。

#### Scenario: 重新武装早于防抖到期时仍不落盘

- **WHEN** 用户在清除应用数据之前编辑过引导 URL，清除完成（写入重新武装）早于该编辑
      的防抖窗口到期
- **THEN** 该 URL MUST NOT 落盘，存储值 MUST 保持清除后的空值

#### Scenario: 清除之后的新编辑照常落盘

- **WHEN** 用户在清除应用数据之后再次编辑引导 URL
- **THEN** 该 URL MUST 在防抖窗口之后落盘，且 `latestBootstrapUrlEdit()` 与落盘值一致
