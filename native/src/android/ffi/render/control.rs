//! 渲染控制类 JNI 导出：暂停/恢复渲染器与测试用的 surface 失效注入。
//!
//! 这两个导出此前寄居在字体模块里，于是 logcat 的 tag 是
//! `native::android::ffi::font`——把渲染开关记成字体事件，排障时按 tag 过滤必然漏掉。
//! 日志 tag 取自 `log` 宏的模块路径（`native/src/android/logging.rs` 用
//! `record.target()` 作 tag），故归位到渲染模块即可让 tag 与语义一致，无需任何
//! 过滤或降级。

use super::super::{
    EnvUnowned, JClass, ThrowRuntimeExAndFailure, jboolean, jlong, render_state_mut,
    set_render_paused_flag,
};

/// 开启/关闭持续的 surface 级取纹理失败（见 `Renderer::set_surface_loss_injected_for_test`）。
///
/// 仅供仪器化用例复刻「BufferQueue 被遗弃」以验证自愈：无生产调用方；关闭后立刻
/// 恢复真实取纹理。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSurfaceLossInjected<
    'local,
>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    injected: jboolean,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, false, |_env| {
        let mut state = render_state_mut();
        match state.as_mut() {
            Some(render_state) => {
                render_state
                    .renderer
                    .set_surface_loss_injected_for_test(injected);
                true
            }
            None => false,
        }
    })
}

/// 暂停/恢复渲染器（如设置界面打开期间或 surface 已销毁时）。Rust 渲染器已在
/// `render_frame` 中检查 `render_paused`；本 JNI 导出补上缺失的传输通道。
///
/// 暂停标志挂在**唯一的全局渲染器**上，与会话无关，故不收会话 id：调用方
/// 曾经按会话逐个调用（每次都要抢被整帧 GPU 工作持有的渲染状态锁），
/// 而零会话时又永远清不掉标志——下一个会话会渲染纯黑。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRenderPaused(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    paused: jboolean,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        // 原子镜像先落：旁路线程（剪贴板应答线程）据此判定「宿主是否还在渲染」，
        // 它不能去抢被整帧 GPU 工作持有的渲染状态锁。
        set_render_paused_flag(paused);
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.renderer.set_render_paused(paused);
            // 恢复时强制全量重绘：暂停期消费的 New 帧永不呈现
            // （`render_frame` 直接返回），且 `last_frame` 已是新数据。
            // 只置 `dirty=true` 会走 Idle 稀疏路径（光标行 + 高亮行），暂停期写入的
            // 内容行不在 bands 内、被 Load 残留覆盖——“隐藏后仍不可见、滑动后部分
            // 出现”。故走 `frame_invalidated` 全量路径。
            if !paused {
                render_state.renderer.frame_invalidated = true;
            }
            log::info!("setRenderPaused: paused={paused}");
        }
    });
}
