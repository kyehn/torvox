# 任务：收尾措辞与实现对齐

## 1. 规范 delta

- [x] 把 `specs/pty-lifecycle/spec.md` 的 delta 写成 MODIFIED Requirements
     （收尾对象 + 宽限数字两处）。
- [x] 归档后核对 `openspec/specs/pty-lifecycle/spec.md` 确实含有新条目数
     （只凭 `openspec validate` 通过不算落地）。

## 2. 验证

- [x] `openspec change validate 2026-10-12-fix-pty-lifecycle-wording` 通过。
- [x] `grep -rn subtree_leader openspec/specs/pty-lifecycle/` 零命中。
- [x] `grep -rn 200ms openspec/specs/pty-lifecycle/` 零命中。
- [x] 实现零改动：`git status --short native/ android/` 为空。
