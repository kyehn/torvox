# Tasks

## 1. 修饰键按下反馈

- [x] 1.1 缩放与底色的动画 spec 增加 40ms 延迟，仅在按下方向生效；抬手复位不延迟。
      验证：`testDebugUnitTest` 中 `ModifierBarRobolectricTest` 全绿。
- [x] 1.2 删除不再使用的 spring 常量与 import。

## 2. 搜索居中

- [x] 2.1 `TerminalSurface.centerSearchRow`：先居中滚动，命中仍在屏幕上边界之外时
      拉满显出量并重新居中。
      验证：`testDebugUnitTest` 中 `TerminalSurfaceLogicTest` 全绿。
- [x] 2.2 `TerminalScreen.centerSelectedRow` 改走新入口。

## 3. 文档

- [x] 3.1 `docs/STATUS.md` 移除两条已解决项。
      验证：模拟器上 `ModifierBarTest` 与 `TextSearchEndToEndTest` 共 24 条仪器用例全绿。

## 4. 全量验证

- [x] 4.1 `./gradlew :app:testDebugUnitTest` 与 `:app:lintDebug` 通过。
      验证：`nu scripts/check.nu` 全量门禁退出码 0。
- [x] 4.2 `openspec validate 2026-10-27-status-open-items-round2 --strict` 通过。

## Workflow follow-up

- 归档本变更，把两条新增要求并入对应规范。
