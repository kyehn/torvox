//! 跨模块集成测试（由 `native/tests/` 移入）：端到端行为只走公共 API，
//! 随单元测试二进制一并运行（`#[cfg(test)]` 门控，不进正式产物）。

pub(crate) mod session_tests;
pub(crate) mod terminal_render_test;
