# 任务

- [x] 建 change（proposal/design/tasks，skip_specs）
- [x] 提取 `cell_codepoint` 并让两行函数共用，`nix develop` 下 `cargo test read_all_text` 通过（2 例通过）
- [x] 收敛 `mark_grid_dirty` 替换四处双写，`cargo clippy` 零警告（fmt 通过，aislop 仅剩长函数/大文件既有告警）
- [x] 外部依赖扫描复核并登记结论（LRU/字体/XML/JSON/base64/防抖/序列化/VT 均已走外部库，本轮两 helper 仅复用 ghostty `grid_ref` 外部 API，无新增可替手搓）
- [x] 查询发送收敛：`Rows/Cols/CursorX/CursorY/ModeGet` 手写 `tx.send` 改走 `try_send`，`cargo clippy -D warnings` 通过，`read_all` 2 例通过，已推送 `b78f7280`
- [x] 逐个复核 CI 16 失败中可在非保护文件修复项，小步提交推送，本地 `cargo test` 定向验证
      ——**2026-10-05 订正**：该 16 失败已由 ～逐条定位修复（见
      `audit-backlog-open-items/tasks.md` §21～§28），保护文件相关项单列于该台账 §5
      待授权；本条按此结项。
- [x] `openspec validate` 通过后归档
