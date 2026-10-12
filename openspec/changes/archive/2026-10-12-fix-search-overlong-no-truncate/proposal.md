# 超长查询不截断，直接判无命中

## Why

`text-search` 规定「needle 超过 128 字符直接判为无命中，MUST NOT 截断——
截断会让用户搜到另一个串的结果」。但 UI 侧 `onQueryChange` 用
`newQuery.take(searchQueryMaxLength)` 先截断（`TerminalScreen.kt:638`），
原生永远收不到超长串：

- spec 的「超长判无命中」在真实链路不可达；
- 用户输入 129+ 字符时搜到的是前 128 字符的结果，正是 spec 禁止的
  「搜到另一个串的结果」。

原生侧的超长→空 needle→总数 0 链条已有测试（`search_empty_needle_returns_nothing`
断总数与视口双 0），实现是对的，错的是 UI 侧的截断。

## What Changes

- `onQueryChange` 不再 `take` 截断：超长查询直接置 `resultCount = 0`、
  不发起 `performSearch`（不送原生）；查询框仍显示用户输入的原文。
- `text-search` spec 增补：UI 侧 MUST NOT 截断查询串，超长 MUST 直接判无命中。

## Capabilities

### Modified Capabilities

- `text-search`：超长查询在 UI 侧直接判无命中，不截断、不送原生。

## Non-goals

- 不改原生侧上限（128 与空 needle 语义不动）。
- 不改防抖、居中、导航行为。
- 不碰 `docs/specification/`。
