//! 合并后的原生 crate：终端引擎、GPU 渲染器与 JNI 桥接，无 feature 开关。
//!
//! `terminal/` 为 Ghostty VT 解析、PTY 管理与 Session；`render/` 为 wgpu 管线与
//! 字形渲染；`android/` 为 JNI FFI 导出、NDK 桥接与日志。单元测试就近存放于
//! `#[cfg(test)]`，集成行为由 `tests/bdd/`（Cucumber + Gherkin）统一管理。

pub mod event;

// ── 终端引擎 ───────────────────────────────────────────────────────────
pub mod terminal;

// ── GPU 渲染器 ──────────────────────────────────────────────────────────
pub mod render;

// ── Android JNI 桥接 ────────────────────────────────────────────────────
pub mod android;

#[cfg(test)]
mod prop_tests;
