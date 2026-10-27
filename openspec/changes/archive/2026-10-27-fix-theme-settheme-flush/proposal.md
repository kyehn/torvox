# Proposal

## Why

`setTheme` 之前只把换主题命令送入 VT 队列，同时立刻把渲染器标脏并整幅重绘。渲染线程
因此可能在 VT 还未推送换主题后的单元数据时，用旧单元数据 + 新背景色重绘一帧，造成新旧
底色混屏（真机 logcat 2026-10-25 采样里夜间画面仍残留日间底色像素）。

## What Changes

- 原生 `setTheme` 在送入 VT 命令后先 `flush()`，等待 VT 线程完成主题切换并推送新单元数据。
- flush 完成后再更新渲染器背景色、失效实例缓存并标脏/整幅重绘，避免 premature 旧帧。
- 增加 VT 单元测试，钉住「切换主题必须重着色已有单元」。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无（行为要求已在 `theme-configuration` 中声明，本次只修正实现时序）。

## Impact

- `native/src/android/ffi/font.rs`：`Java_..._setTheme` 调整 VT flush 与时序。
- `native/src/terminal/ghostty_terminal/tests/render_cache.rs`：新增回归测试。
