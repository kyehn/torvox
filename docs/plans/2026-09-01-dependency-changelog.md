# Dependency Changelog — 2026-09-01 (Master Optimization v8)

> 自动生成于 v8 收口

## Rust 依赖变更

### 已滚动

| crate | 旧版本 | 新版本 | 变更类型 |
|-------|--------|--------|----------|
| smallvec | 1.15.2 | 1.16.0 | patch |

### 新增（v7 硬化引入）

| crate | 版本 | 引入原因 | 使用者 |
|-------|------|----------|--------|
| shell-words | 1.1 | MCP argument tokenization | native/src/mcp/cmd.rs |

### 保持不变（核心依赖）

| crate | 版本 | 说明 |
|-------|------|------|
| wgpu | 25.0.2 | GPU 渲染 |
| nix | 0.30.1 | PTY/unix syscall |
| thiserror | 2.0.12 | 错误类型 |
| memmap2 | 0.9.8 | 文件映射 |
| bytemuck | 1.23.4 | 安全字节转换 |

### 审计结果

```
cargo machete: 0 unused
cargo audit: 0 CVE, 2 allowed unmaintained
  - paste 1.0.15 (allowed)
  - ttf-parser 0.21.1 (allowed)
```

## Kotlin/Gradle 依赖变更

无新增依赖。保持不变：

- compose-bom: 2025.05.01
- kotlinx-coroutines: 1.10.2
- kotlinx-serialization: 1.9.0
- agp: 9.1.0

## 体积影响

| 产物 | 变更前 | 变更后 | Δ |
|------|--------|--------|---|
| libnative.so (arm64) | — | 待构建验证 | — |
| APK | — | 待构建验证 | — |
