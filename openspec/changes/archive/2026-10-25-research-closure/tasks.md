# Tasks

## 1. 两条判据

- [x] 1.1 增量构建与全量构建逐字节一致，验证：`cached_build_matches_full_build_byte_for_byte`
- [x] 1.2 resize 循环不累积旧网格的行，验证：`resize_cycles_do_not_accumulate_stale_rows`

## 2. 三条口径成文

- [x] 2.1 真机测量操作规程（p95/p99、同条件交替、环境记录、模拟器边界）落
      `performance-measurement`
- [x] 2.2 基准负载口径固定为合成负载（不采纳真机 PTY 捕获重放）落同一能力
- [x] 2.3 不采纳项（atrace counter 关联）记录理由，验证：规范含该条

## 3. 规范

- [x] 3.1 新能力 `performance-measurement` 与两个能力的增量落在本 change，
      验证：`openspec validate 2026-10-25-research-closure --strict`
- [x] 3.2 归档后 `openspec validate --all --strict` 全绿

## 4. 研究文档收口

- [x] 4.1 六份 `docs/research/*.md` 中已解决的「可参考点 / 行动建议 / 观察项」移除
- [x] 4.2 每份文档保留指向 openspec 能力的归属说明，研究结论原文不动
- [x] 4.3 `markdownlint-cli2 "**/*.md"` 无新增告警

## 5. 集成验证

- [x] 5.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib`
- [x] 5.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck`
