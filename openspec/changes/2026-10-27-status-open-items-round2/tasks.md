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

- [ ] 3.1 `docs/STATUS.md` 移除两条已解决项。
      未执行：沙盒无真机，修复尚未经仪器测试验证，暂不删除问题条目。

## 4. 全量验证

- [ ] 4.1 `./gradlew :app:testDebugUnitTest` 与 `:app:lintDebug` 通过。
      沙盒缺少 Android SDK/Gradle，未执行；须在 `nix develop` 内运行。
- [ ] 4.2 `openspec validate 2026-10-27-status-open-items-round2 --strict` 通过。
      `openspec` 由 `flake.nix` 提供，沙盒无 nix，未执行。

## Workflow follow-up

- 归档本变更，把两条新增要求并入对应规范。
