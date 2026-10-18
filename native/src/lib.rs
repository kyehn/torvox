//! 合并后的原生 crate：终端引擎、GPU 渲染器与 JNI 桥接，无 feature 开关。
//!
//! `terminal/` 为 Ghostty VT 解析、PTY 管理与 Session；`render/` 为 wgpu 管线与
//! 字形渲染；`android/` 为 JNI FFI 导出、NDK 桥接与日志。单元测试就近存放于
//! `#[cfg(test)]`，集成行为由 `tests/bdd/`（Cucumber + Gherkin）统一管理，
//! 并发不变量由 `tests/concurrency.rs`（shuttle）单独承载。

pub mod event;

// ── 终端引擎 ───────────────────────────────────────────────────────────
pub mod terminal;

// ── GPU 渲染器 ──────────────────────────────────────────────────────────
pub mod render;

// ── Android JNI 桥接 ────────────────────────────────────────────────────
pub mod android;

/// 取 panic 载荷的可读文本（`&str` / `String`，其它类型退化为占位符）。
///
/// 唯一一处把 panic 载荷映射成文本的实现，三处 panic 现场共用：JNI 导出守卫
/// （`android::ffi`）、ghostty VT 线程的 spawn 失败路径
/// （`terminal::ghostty_terminal`）与 Android surface 重建（`render::context`）。
/// 各现场对载荷的**销毁**策略不同（解卷路径故意泄漏，见
/// `android::ffi::take_panic_text_and_leak`），故这里只借用、不负责销毁。
///
/// 必须给 `&'static str` 显式标注类型：`panic!` 的静态字面量载荷正是该类型，
/// 若写成裸 `&str`，推断出的生命周期可能与载荷的 `TypeId` 对不上而让 downcast
/// 落空——后果是每个真实 panic 的文本都被报成占位符。
pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&'static str>() {
        return (*text).to_string();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    "non-string panic payload".to_string()
}

#[cfg(test)]
mod panic_text_tests {
    use super::panic_text;

    /// 载荷的三种真实形态：静态字面量、`String` 与未知类型。
    ///
    /// 判别力：把 `downcast_ref::<&'static str>` 写成推断生命周期的 `&str`，
    /// 第一条断言即落空成占位符（见 `panic_text` 的文档注释）。
    #[test]
    fn payload_shapes_all_map_to_readable_text() {
        assert_eq!(
            panic_text(&"GPU initialization failed"),
            "GPU initialization failed"
        );
        assert_eq!(panic_text(&String::from("索引越界")), "索引越界");
        // 非字符串载荷：断言的是「仍能给出一条可读诊断」这条性质，而不是把兜底那句
        // 文案抄回来——后者只会复述实现。空串会让 panic 诊断失去意义，Debug 形式
        // （"42"）虽可读但与真正的 panic 文本混淆。
        let fallback = panic_text(&42_u32);
        assert!(!fallback.is_empty(), "非字符串 panic 载荷必须给出非空诊断");
        assert_ne!(fallback, "42", "非字符串载荷不得直接回显 Debug 形式");
    }
}
