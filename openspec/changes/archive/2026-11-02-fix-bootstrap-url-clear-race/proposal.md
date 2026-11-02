# Proposal

## Why

「清除应用数据」有一条明确的防串味承诺：**清除之前编辑的引导 URL MUST NOT 在清除
之后落盘**。否则用户看到的「已清除」提示是假话——刚清掉的 URL 又被写回
`preferences_pb`，设置页回填出清除前的旧值。

仓库里的实现试图兑现这条承诺，但兑现得并不完整：

- `dropPendingBootstrapUrlEdits()` 把「防抖写入」停用（`bootstrapUrlEditsArmed = false`）
  并清空 replay 缓存；
- `TerminalViewModel.clearAppData()` 在删除目录的 `finally` 里调用
  `rearmBootstrapUrlEdits()` 把写入重新武装。

问题出在**停用窗口的边界**：停用只覆盖「删除目录并重建 prefs」这段耗时，而防抖写入
的到期时刻是**上一次编辑之后的 300ms**，两者互不相干。清除耗时一旦长于剩余的防抖
窗口（删除含引导 zip 的 cache 目录很容易超过 300ms），待写值就在写入重新武装**之后**
才到期——收集器此时看到的是「已武装」，于是把清除前的 URL 原样写回。

代码里那句注释把这个不变量写得很死：

> 防抖窗口内（300ms）的编辑不可能在删除之后落地。

而它并不成立。既有回归钉 `dropped debounce edit never lands, rearmed edit does` 也没有
覆盖这条路径：它先睡满 700ms（让防抖在**停用期间**到期）才重新武装，测的恰好是
「停用窗口覆盖了到期时刻」这一种时序。

实测（改动前）：

```text
recordBootstrapUrlEdit("https://…/dropped.zip")
dropPendingBootstrapUrlEdits()
rearmBootstrapUrlEdits()   // 清除结束，比防抖到期早
Thread.sleep(700)          // 防抖此刻到期
→ preferences_pb 里写回了 dropped.zip
```

## What Changes

- 引导 URL 的防抖流元素从裸字符串改为「值 + 清除轮次」（`BootstrapUrlEdit`）。
- `dropPendingBootstrapUrlEdits()` 递增轮次：清除之前编辑的待写值就此作废，
  **无论它何时到期**。
- 落盘判据加上轮次比对：只有「轮次与当前一致」的编辑才写盘。清除之后的新编辑
  取当前轮次，照常落盘。
- `rearmBootstrapUrlEdits()` 的语义不变（清除结束后重新接受写入），但它不再能让
  清除之前的待写值复活——作废由轮次保证，不由停用窗口的长度保证。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `settings-propagation`：新增一条 Requirement，把「清除应用数据作废清除前的待写
  编辑」写成可核对的判据——此前只体现为一句实现注释与一条覆盖不到该时序的用例。

## 不做什么

- 不改防抖窗口（300ms）：窗口长短与本缺陷无关，缺陷在于「作废」只靠停用窗口。
- 不取消 `rearmBootstrapUrlEdits()`：清除后写入必须重新被接受，否则用户下一次编辑
  永不落盘。
- 不改 `docs/specification/`（禁改区）与 `docs/STATUS.md`（文件首行明禁修改）。

## Impact

- Affected specs: `settings-propagation`（一条 ADDED Requirement）。
- Affected code:
  - `android/app/src/main/java/terminal/emulator/settings/SettingsRepository.kt`
    （防抖流元素类型、轮次、落盘判据）
  - `android/app/src/test/java/terminal/emulator/settings/SettingsRepositoryTest.kt`
    （一条回归用例）
