use super::*;

#[cfg(test)]
mod surface_format_selection {
    use super::Renderer;
    use wgpu::{
        CompositeAlphaMode, PresentMode, SurfaceCapabilities, TextureFormat, TextureUsages,
    };

    fn caps(formats: Vec<TextureFormat>) -> SurfaceCapabilities {
        SurfaceCapabilities {
            formats,
            format_capabilities: Vec::new(),
            present_modes: vec![PresentMode::Fifo],
            alpha_modes: vec![CompositeAlphaMode::Auto],
            usages: TextureUsages::RENDER_ATTACHMENT,
        }
    }

    /// 回归钉：必须指名要 8 位 RGBA 族，而不是「取第一个非 sRGB 格式」。
    ///
    /// 真机（Mali GPU）在 `attachWindow` 后立刻报告 R8 格式不被分配器支持——0x38 就是
    /// `AHARDWAREBUFFER_FORMAT_R8_UNORM`。旧写法把「设备排第一的非 sRGB 格式」直接
    /// 当交换链格式，于是照着设备的首选去申请窗口缓冲；Mali 的 gralloc 分配不出来，
    /// 即便侥幸分配成功，渲染进 R8 目标也只有灰度一个通道。
    #[test]
    fn rgba8_is_preferred_even_when_the_device_lists_r8_first() {
        let selected = Renderer::select_surface_format(&caps(vec![
            TextureFormat::R8Unorm,
            TextureFormat::Rgba8Unorm,
        ]))
        .expect("candidate present");
        assert_eq!(selected, TextureFormat::Rgba8Unorm);
    }

    /// 只报 sRGB 的 RGBA8 时退到它，而不是随手挑单通道格式。
    #[test]
    fn falls_back_to_srgb_rgba8_when_plain_rgba8_is_absent() {
        let selected = Renderer::select_surface_format(&caps(vec![
            TextureFormat::R8Unorm,
            TextureFormat::Rgba8UnormSrgb,
        ]))
        .expect("candidate present");
        assert_eq!(selected, TextureFormat::Rgba8UnormSrgb);
    }

    /// 一个候选都没有时必须报「无格式」而不是默默选一个。
    #[test]
    fn empty_format_list_is_an_error() {
        assert!(Renderer::select_surface_format(&caps(Vec::new())).is_err());
    }
}

#[cfg(test)]
mod send_check {
    #[test]
    fn renderer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<super::Renderer>();
        fn assert_sync<T: Sync>() {}
        assert_sync::<super::Renderer>();
    }
}

#[cfg(test)]
mod gpu_error_classification {
    use super::gpu_error_is_surface_lost;

    fn validation(description: &str) -> wgpu::Error {
        wgpu::Error::Validation {
            source: Box::new(std::io::Error::other("configure failed")),
            description: description.to_string(),
        }
    }

    /// 平台收走窗口后 wgpu 重新配置 surface 必然报「不支持该队列族」——
    /// 这一类是平台事件的连带结果，不得按应用缺陷（error）记。
    #[test]
    fn surface_lost_reconfigure_is_not_an_app_defect() {
        let error = validation(
            "Validation Error\n\nCaused by:\n  In Surface::configure\n    \
             Surface does not support the adapter's queue family\n",
        );
        assert!(gpu_error_is_surface_lost(&error));
    }

    /// 自家配置错误（格式不支持等）仍是应用缺陷，必须按 error 记。
    #[test]
    fn genuine_configure_error_stays_an_error() {
        let error = validation(
            "Validation Error\n\nCaused by:\n  In Surface::configure\n    \
             Requested format Rgba8Unorm is not in list of supported formats: []\n",
        );
        assert!(!gpu_error_is_surface_lost(&error));
    }

    /// 非校验错误（内部错误）即便文案相同也不属「窗口被收回」。
    #[test]
    fn non_validation_errors_are_not_surface_lost() {
        let error = wgpu::Error::Internal {
            source: Box::new(std::io::Error::other("boom")),
            description:
                "In Surface::configure\n    Surface does not support the adapter's queue family\n"
                    .to_string(),
        };
        assert!(!gpu_error_is_surface_lost(&error));
    }
}

#[cfg(test)]
mod attach_surface_reuse {
    use super::reuse_attached_surface;

    /// 回归钉：快速路径必须要求**同一个** `ANativeWindow`。
    ///
    /// 真机日志里回前台、加会话、切会话每次都送新窗口，而缓存
    /// surface 指向的旧 BufferQueue 已被回收：放行快速路径就 reconfigure 出一个死
    /// 交换链，随后每帧 `dequeueBuffer: BufferQueue has been abandoned` →
    /// `ERROR_SURFACE_LOST_KHR` → 连续两次取帧失败判死 → 宿主重建 `SurfaceView`，
    /// 期间屏幕停在旧会话或纯主题背景那一帧（用户可见「切换无变化」）。
    #[test]
    fn fast_path_needs_the_same_window_and_a_live_surface() {
        assert!(reuse_attached_surface(
            true,
            true,
            false,
            Some(0x1000),
            0x1000
        ));
        // 换窗口（回前台 / 新会话 / 切会话）：必须重建。
        assert!(!reuse_attached_surface(
            true,
            true,
            false,
            Some(0x1000),
            0x2000
        ));
        // 尚未挂载过窗口：没有可复用的 surface。
        assert!(!reuse_attached_surface(true, true, false, None, 0x1000));
        // 已判死的 surface 是死队列，reconfigure 不会复活它。
        assert!(!reuse_attached_surface(
            true,
            true,
            true,
            Some(0x1000),
            0x1000
        ));
        // 没有 surface 或没有配置：无物可复用。
        assert!(!reuse_attached_surface(
            false,
            true,
            false,
            Some(0x1000),
            0x1000
        ));
        assert!(!reuse_attached_surface(
            true,
            false,
            false,
            Some(0x1000),
            0x1000
        ));
    }
}

#[cfg(test)]
mod in_flight_rebuild_guard {
    use super::rebuild_blocked_by_in_flight_surface;

    const WINDOW: usize = 0x7f00_1234;

    /// 判定表：排干成功一律放行；排干失败只拒绝「重建目标正是仍有克隆在途的那个窗口」。
    ///
    /// 拒绝分支对应 2026-10-18 的崩溃现场：`release_surface` 后 53ms 宿主把**同一个**
    /// （同址）`ANativeWindow` 送回来重建，而在途取帧仍持有旧 surface 的克隆——
    /// `vkCreateAndroidSurfaceKHR` 报 `ERROR_NATIVE_WINDOW_IN_USE_KHR`，
    /// wgpu-hal `.expect()` panic 沿 JNI 边界把整个进程带走。
    #[test]
    fn only_a_same_window_rebuild_is_refused_while_an_acquire_is_in_flight() {
        let cases: &[(bool, Option<usize>, Option<usize>, usize, bool)] = &[
            // 排干成功：一律放行（正常路径）。
            (true, Some(WINDOW), Some(WINDOW), WINDOW, false),
            (true, None, None, WINDOW, false),
            // 排干失败 + 目标正是当前挂载的窗口（未释放即在途）：拒绝。
            (false, Some(WINDOW), None, WINDOW, true),
            // 排干失败 + 目标正是刚释放的窗口（release→attach 同址）：拒绝。
            (false, None, Some(WINDOW), WINDOW, true),
            // 排干失败 + 换了新窗口：陈旧克隆包着旧 BufferQueue，放行。
            (false, Some(WINDOW), Some(WINDOW), WINDOW + 0x1000, false),
            (false, None, Some(WINDOW), WINDOW + 0x1000, false),
            // 排干失败却既无挂载也无释放记录：本不可达（在途克隆必来自挂载过的 surface），
            // 保守放行且由上层日志暴露，避免守卫在这里凭空拒绝一切重建。
            (false, None, None, WINDOW, false),
        ];
        for &(drained, attached, retired, incoming, expected) in cases {
            assert_eq!(
                rebuild_blocked_by_in_flight_surface(drained, incoming, attached, retired),
                expected,
                "drained={drained} attached={attached:?} retired={retired:?} incoming={incoming:#x}",
            );
        }
    }
}
