# Proposal

## Why

CI run 在 `connectedDebugAndroidTest` 16 例失败，jscpd 实测重复率 2.22%（Rust 3.52%），需以最小实现消除重复并用外部 API 替代手写逻辑。

## What Changes

- 提取单元格码点读取公共 helper，消除 `read_all_text_row_impl` 与 `read_line_text_impl` 的重复逐格 FFI。
- 收敛 `cached_all_text` 失效与 `grid_dirty` 置位为单一 helper，消除四处散落的双写。
- 外部依赖扫描：路径消解已用 `java.nio.file`，JSON 用 kotlinx.serialization，base64 用平台 API，VT 用上游 ghostty，无新增手搓可替项则不强换。
- 修复 CI 16 失败中可在非保护文件内修复的产品侧根因（测试侧重试已在 70b5c009..HEAD 落地 7 例，余下 9 例逐个小步修复），不碰保护文件，不加未声明 Fallback，失败必须出声。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无

## Impact

- `native/src/terminal/ghostty_terminal/internal.rs`、`commands.rs`、`public_api.rs`、`ffi.rs` 纯重构，行为不变。
- `android/app/src/androidTest` 仅加固等待与门控，不改产品语义。
- 不改 `.github/`、`scripts/`、`flake.nix`、`*.gradle.kts`、`detekt.yml`、`docs/specification/` 等保护文件。
