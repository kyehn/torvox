//! 合并后的原生 crate：终端引擎、GPU 渲染器与 JNI 桥接，无 feature 开关。
//!
//! `terminal/` 为 Ghostty VT 解析、PTY 管理与 Session；`render/` 为 wgpu 管线与
//! 字形渲染；`android/` 为 JNI FFI 导出、NDK 桥接与日志。单元测试就近存放于
//! `#[cfg(test)]`，并发不变量由 `tests/concurrency.rs`（shuttle）承载。

pub mod event;

// 跨层共用的诊断限频闸门：渲染侧慢帧诊断、终端侧帧丢弃告警、事件队列溢出告警
// 同源（见 `slow_log`）。
pub(crate) mod slow_log;

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

/// 把多行文本压成单行：行间以 ` | ` 相接，空行丢弃，行首尾空白裁掉。
///
/// 诊断文本常自带缩进与换行树（wgpu 的错误文本即如此：正文之后跟一段
/// `Caused by:` 原因树，中间还夹着空行）。按行采集的日志通道把**空行**当作
/// 记录分隔，载荷里的空行之后的内容就此消失——真机日志里
/// `Error in Device::poll: Validation Error` 之后只剩空白，判死原因
/// `Parent device is lost` 整段丢失，而那正是唯一能指认原因的文字。
///
/// 故诊断正文一律先压成单行：内容一条不少，只是以 ` | ` 相接。
pub(crate) fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// panic 记录的单行正文：`panicked at 位置: 单行化载荷`。
///
/// 载荷里的空行会让按行采集的日志通道在空行处截断记录，原因链随之丢失
/// （见 [`one_line`]）。此处把载荷压平后与位置拼成一行：判死原因仍完整
/// 留在正文里，只是与其它行以 ` | ` 相接。
pub(crate) fn panic_record_text(
    payload: &str,
    location: Option<&std::panic::Location<'_>>,
) -> String {
    let location = location.map_or_else(|| "unknown location".to_string(), ToString::to_string);
    format!("panicked at {location}: {}", one_line(payload))
}

/// 进程内单调时钟读数（纳秒，自进程首次调用起算）。
///
/// 忙标记（VT 忙、输出投递忙）存的是「自忙起时刻」的读数，判定方取一次当前读数
/// 相减即得挂起时长。两个标记必须同源：各自取基准则相减无意义。
///
/// 读数恒为正：忙标记以 0 表示「空闲」，基准刚建立时 `elapsed` 可能仍是 0，
/// 那会让「刚开始忙」被读成空闲。
pub(crate) fn monotonic_nanos() -> u64 {
    static BASE: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let elapsed_nanos = BASE
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos();
    elapsed_nanos.max(1) as u64
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

#[cfg(test)]
mod panic_record_tests {
    use super::{one_line, panic_record_text};

    /// wgpu 的判死载荷：正文与原因树之间夹着空行。
    ///
    /// 回归钉：压平后原因必须仍在正文里，且正文不含换行——按行采集的日志通道
    /// 会在空行处截断记录，原因随之消失（真机日志只剩「Validation Error」）。
    #[test]
    fn device_loss_payload_keeps_its_cause_on_one_line() {
        let payload =
            "Error in Device::poll: Validation Error\n\nCaused by:\n  Parent device is lost";
        let text = panic_record_text(payload, None);
        assert!(!text.contains('\n'), "panic 正文不得含换行：{text}");
        assert!(text.contains("Caused by"), "原因链的引导词必须留在正文：{text}");
        assert!(
            text.contains("Parent device is lost"),
            "判死原因必须留在正文：{text}"
        );
    }

    /// 空行不占位、行首尾空白裁掉：压平后的正文仍是可读的一句话。
    #[test]
    fn one_line_drops_blank_lines_and_trims() {
        assert_eq!(one_line("a\n\n  b  \n"), "a | b");
    }
}
