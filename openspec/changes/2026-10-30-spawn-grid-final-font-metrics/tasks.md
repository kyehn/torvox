# Tasks

## 1. 字体契约在 spawn 之前落定

- [x] `Bridge` 的字体相关入口（`setFontFamily`/`setFontSizeInPlace`/`setRasterScale`/
      `loadFontFile`）改为全局下发，不再走会话闸
- [x] `spawnTerminalTimed` 在读度量之前下发字体契约（字族 + 字号 + 光栅尺度）
- [x] spawn 与契约下发共用同一份字族（解析一次）

## 2. 规范

- [x] `startup-grid-resize` 补上「算网格的度量必须已落定」的前提与场景

## 3. 验证

- [x] `InitialSpawnGridTest` 补上 spawn 网格与重算网格相等的用例
- [x] `:app:testDebugUnitTest --tests terminal.emulator.runtime.InitialSpawnGridTest` 通过
- [x] `Bridge.kt` / `TerminalRuntime.kt` 通过 detekt 与 spotless
