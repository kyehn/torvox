# Design

## Context

`read_all_text_row_impl` 与 `read_line_text_impl` 各自逐格走 `grid_ref` + `cell` + `codepoint` 三段 FFI，差异仅在空单元处理（跳过 vs 空格占位）。`cached_all_text = None` 与 `grid_dirty = true` 在 Write/SetTheme/Resize/Reset 四处成对出现。

## Goals / Non-Goals

**Goals:**

- 同一码点读取只写一次，语义分支保留在调用方。
- 网格脏标记与文本缓存失效单点收敛，后续新增失效只需调一处。
- 不新增 JNI，不改查询超时，不加静默回退。

**Non-Goals:**

- 不改 `render_inner` 空闲语义，不改搜索列号占位契约。
- 不引入新依赖，扫描结论复用 /。

## Decisions

- 新增 `cell_codepoint(terminal, row, col)` 直构 `Point::Screen`，不逐格查 `grid_cols`，两处行函数共用。
- 新增 `mark_grid_dirty(grid_dirty, cached_all_text)` 置脏并清文本缓存，替换四处双写。
- 测试侧重复重试不抽公共 helper：三处重试的等待对象与断言口径不同，强抽会藏语义，保持现状。

## Risks / Trade-offs

- [Risk]  helper 抽取后行函数仍有 trim 差异 → Mitigation：差异是产品语义（搜索占位 vs 全量跳空），保留分支并由既有单测锁定。
- [Risk] 脏标记收敛后漏清 → Mitigation：helper 内双写原子，调用方无法只置其一。

## Migration Plan

- 无迁移；回滚即还原两 helper，行为由 `read_all_text` 单测与既有 544 单测验证。
