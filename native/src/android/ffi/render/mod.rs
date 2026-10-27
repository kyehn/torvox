//! 渲染路径的 JNI 导出：会话推进、帧呈现、表面挂载、剪贴板应答与会话标题查询。

mod clipboard;
mod diagnostics;
mod frame;
mod overlay;
mod pump;
mod session_ops;
mod window;

// 以下再导出只服务宿主单测：生产代码不消费它们，故限定为测试构建。
#[cfg(test)]
pub(super) use diagnostics::{
    ALT_SCREEN_ACTIVE_BIT, LAST_CONTENT_ROW_NONE_BITS, RENDER_SAMPLE_FAILURE_BITS,
    SURFACE_INVALIDATED_BIT, pack_render_sample,
};
#[cfg(test)]
pub(super) use overlay::{
    build_cursor, frame_reflects_scroll, highlight_rows_to_mark, mark_overlay_dirty_rows,
    should_repaint_idle,
};
pub(super) use pump::pump_sessions;
pub(super) use session_ops::{ATLAS_SIZE, DEFAULT_FONT_CELL_SIZE, string_to_jstring};
