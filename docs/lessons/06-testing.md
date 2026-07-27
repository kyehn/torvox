# 测试经验教训

## 1. 测试质量审计 — 82 个无效测试被清除

### 背景
在一次全面代码审计中，发现了大量"无效测试"(dud tests)。这些测试通过了 CI 但实际没有任何验证价值。清除行动在 10 个测试文件中替换了 82 个无效测试、强化了 112 个薄弱测试。

### 无效测试的类型

| 类型 | 描述 | 示例 |
|------|------|------|
| **循环构造** | 构造数据后断言必然为真的值 | 构造 Cell 然后断言其字段存在（刚刚构造完当然存在） |
| **零断言** | 只是不崩溃就算通过 | 调用一个函数但没有断言任何返回值或副作用 |
| **宽松断言** | 用不精确的范围/近似值 | 用 CJK 宽度"接近"的断言替代具体值 |
| **名字误导** | 测试名与实际内容不符 | `cross_backend` 测试实际没有做跨后端验证 |
| **隐式数据丢失** | 使用 `filter_map` 时丢弃失败数据 | 处理链中丢失了某些数据但未察觉 |
| **非确定性** | 使用随机种子导致每次跑可能不同 | `SystemTime` 做 RNG 种子 → 测试结果依赖时间 |

### 修复方案
- 循环构造和零断言测试 → 替换为有具体断言的测试
- 宽松范围断言 → 替换为精确的 ANSI 调色板值
- 名字误导测试 → 重命名并添加实际断言
- `filter_map` 数据丢失 → 使用 `filter` + `map` 分离，确保不丢失
- `SystemTime` RNG → 替换为 `StdRng::seed_from_u64` 确定性种子

### 关键教训
1. **测试通过不等于测试有效** — CI 通过只能证明测试没有失败，不能证明测试有价值
2. **每个测试必须断言具体行为** — "不崩溃就算通过" 是无效测试
3. **测试名必须反映实际验证内容** — 误导性名称比没有测试更糟糕
4. **使用确定性种子** — 任何随机测试都必须使用固定种子，否则是 flaky test
5. **代码审查者应检查测试质量** — 不仅仅是"测试通过了"
6. **定期审计测试代码** — 和产品代码一样，测试代码也会退化

### 后续行动
- 添加 "dud test detection" 到 CI 门控
- 代码审查时要求测试必须有至少一个 `assert_eq!`/`assert!` 断言
- 优先为已有功能写测试，而不是为测试而测试

### 相关提交
- `1d893cc555`: fix(tests): eliminate 82 dud tests, strengthen 112 weak tests

## 2. 删除78个derive宏测试后被Revert

### 问题
A "test cleanup" deleted 78 tests that only verified derive macros, e.g. `#[test] fn default_debug { let _ = format!("{:?}", Cell::default()); }`. These tests were considered "trivial" because they only verified that derive macros generated the expected trait implementations. The deletion was reverted shortly after due to feedback.

### 教训
Even "trivial" tests provide value: (a) They act as a regression net for unintended behavior changes when derive macros or types change, (b) The cost of maintaining them is near zero, (c) Deleting them provides no meaningful speed or clarity benefit. When in doubt, keep the test.

### 相关提交
- `c29a8289`: revert "test cleanup: remove trivial derive macro tests"
- `c7e93daf`: test cleanup: remove 78 trivial derive macro tests (reverted)

## 3. Android像素验证 → Rust端内部状态验证

### 问题
Tests verified rendering correctness by doing pixel-level analysis of Android screenshots. This approach was slow (needs emulator/device), unreliable (screenshots vary by device/resolution), and far from the data source.

### 修复
Move verification logic to Rust side where it can directly inspect internal Grid state (cell data, cursor position, colors) without waiting for GPU rendering and screenshot capture.

### 教训
Testing strategy should verify as close to the data source as possible. Rust-side state validation is faster (sub-ms vs seconds) and more reliable (deterministic vs pixel variances) than Android-side screenshot analysis. Pixel-level tests are still useful for integration/E2E but not for unit/correctness testing.

### 相关提交
- `a90db152`: refactor(tests): move rendering verification from Android screenshots to Rust Grid state

## 4. scrollbackLine()返回null导致搜索失效 — GhosttyTerminal API 陷阱

### 问题
`performSearch()` used `scrollbackLine()` to iterate terminal content, but that API returns null for most indices — Ghostty only stores lines that have been accessed, not all lines. This caused the search loop to break immediately, resulting in empty `fullText` and always-empty search results.

### 修复
Switched to `getTerminalText()` which returns complete terminal content as a flat string.

### 教训
GhosttyTerminal's `scrollbackLine()`/`readLine()` API has lazy access semantics — only lines that have been explicitly read are retained in accessible form. Don't use it for full text iteration. Use `getTerminalText()` or similar "dump all content" APIs instead.

### 相关提交
- `09355bd6`: fix(search): use getTerminalText instead of scrollbackLine for full content iteration
