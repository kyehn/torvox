# Tasks

- [x] T1: R3 — 替换 `applyScrollDistance` 中 `floor()` 为 `toInt()`（截断趋向零），确保正/负亚行阈值对称
- [x] T2: R3 — `ScrollDistanceTest` 新增：亚行往返等量不漂移、经过零点无跳变、截断对称性
- [x] T3: R4 — `stopFlingAnimation` 中断 fling 时清零余量并发 `onScrollingStateChanged(false)`
- [x] T4: R4 — `ScrollDistanceTest` 新增：fling 行速度符号与拖动方向一致（已有，确认不回归）
- [x] T5: 运行 `ScrollDistanceTest` + `GridToScreenTest` 验证
- [x] T6: openspec archive，git commit
