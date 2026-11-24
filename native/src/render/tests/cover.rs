//! 开孔遮罩的像素回读：回读内容必须与累加器里画的东西一致。
//!
//! 遮罩的用途是「切后台返回不闪一下」：平台在切后台后拆掉窗口 surface，
//! 回前台时交换链必须重建，那几十毫秒里开孔没有缓冲，宿主把回读到的上一帧
//! 画在开孔里。这条测试钉住回读的两件易错事：行填充必须被去掉（否则画面
//! 逐行错位），以及拿到的必须是累加器里的像素（不是别的纹理）。

use super::*;
use crate::render::context::Renderer;

#[test]
fn cover_readback_returns_the_accumulator_pixels() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);
    // 累加器只在交换链支持拷贝时创建（生产路径由 attach_surface 打开）。
    context.swapchain_copy_supported = true;
    let view = context
        .ensure_frame_texture(4, 4, wgpu::TextureFormat::Rgba8Unorm)
        .expect("累加器视图");
    // 把累加器清成不透明红色：回读必须一字不差地拿到它。
    {
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Cover Clear Encoder"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Cover Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }
        context.queue.submit(Some(encoder.finish()));
    }
    // 累加器里已有一帧**已渲染**的内容：回读的前提（与 warmup 呈现保留帧同一判据）。
    context.frame_content_valid = true;
    // 本设备世代已真正出过一帧：回读的另一个前提（见 `has_presented_frame`）。
    context.has_presented_frame = true;

    let cover = context
        .cover_readback_source()
        .expect("回读源")
        .read()
        .expect("遮罩帧");
    assert_eq!((cover.width, cover.height), (4, 4));
    assert_eq!(
        cover.pixels.len(),
        4 * 4 * 4,
        "像素数必须等于宽×高×4（行填充必须已被去掉）"
    );
    assert_eq!(
        &cover.pixels[0..4],
        &[255, 0, 0, 255],
        "回读到的必须是累加器里的像素"
    );
    // 每一行都必须是同一个红色：行填充没去掉时第 1 行之后就会错位。
    for row in 0..4 {
        let start = row * 4 * 4;
        assert_eq!(
            &cover.pixels[start..start + 4],
            &[255, 0, 0, 255],
            "第 {row} 行错位"
        );
    }
}

/// 没有累加器时没有可遮罩的内容：返回 `None` 而不是空帧（空帧会被宿主
/// 当成「有遮罩但画面全黑」，比没有遮罩更糟）。
#[test]
fn cover_readback_without_accumulator_yields_nothing() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let context = setup_test_gpu_context(device, queue);
    assert!(
        context.cover_readback_source().is_none(),
        "没有累加器时不得返回空遮罩"
    );
    let _ = Renderer::select_surface_format;
}

/// 累加器刚建好、内容未定义时不得回读：回读上来是未初始化的显存，
/// 画进开孔比没有遮罩更糟（用户看到的是雪花而不是离开时的画面）。
#[test]
fn cover_readback_of_undefined_accumulator_yields_nothing() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);
    context.swapchain_copy_supported = true;
    context
        .ensure_frame_texture(4, 4, wgpu::TextureFormat::Rgba8Unorm)
        .expect("累加器视图");
    assert!(
        context.cover_readback_source().is_none(),
        "内容未定义的累加器不得被回读"
    );
}

/// 设备判死后不得再给出回读源：死设备上的任何 wgpu 调用都只会换来 wgpu 内部的
/// 一道 panic（`Parent device is lost`），那道 panic 穿出 JNI 边界就是整进程退出
/// （2026-11-12 真机日志）。
///
/// 判别力：少了这道判死检查时，回读会先建缓冲再提交，整条路径都在死设备上跑，
/// 最终以 panic 收场。
#[test]
fn cover_readback_of_a_lost_device_yields_nothing() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);
    context.swapchain_copy_supported = true;
    context
        .ensure_frame_texture(4, 4, wgpu::TextureFormat::Rgba8Unorm)
        .expect("累加器视图");
    // 累加器里已有一帧已渲染的内容：内容判据不是本用例的拦路虎。
    context.frame_content_valid = true;
    context.has_presented_frame = true;
    // 换成独立的判死位：不碰进程级全局，免得影响同批次的其他用例。
    context.lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    assert!(
        context.cover_readback_source().is_none(),
        "判死的设备不得再给出回读源"
    );
}

/// 新设备世代还没出过一帧时不得回读：累加器里的内容是旧设备留下的（重建保留它
/// 供 warmup 呈现），而回读要在新设备上等第一次拷贝完成——Mali-G57 上正是这条
/// 等待触发设备丢失（2026-11-24 真机日志：每次重建后 1-4ms 内 `readCoverFrame`
/// 的轮询报设备丢失）。
///
/// 判别力：少了这道门时，新设备上第一次周期回读就会走完整条回读路径，正是实测
/// 触发丢失的调用。
#[test]
fn cover_readback_before_the_first_presented_frame_yields_nothing() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let mut context = setup_test_gpu_context(device, queue);
    context.swapchain_copy_supported = true;
    context
        .ensure_frame_texture(4, 4, wgpu::TextureFormat::Rgba8Unorm)
        .expect("累加器视图");
    // 累加器里有旧设备留下的已渲染内容，但本设备世代尚未出过帧。
    context.frame_content_valid = true;
    assert!(!context.has_presented_frame);
    assert!(
        context.cover_readback_source().is_none(),
        "本设备世代还没出过帧时不得回读"
    );
}

/// 非阻塞轮询在活设备上必须给「继续」：遮罩回读靠它推进映射回调。
#[test]
fn non_blocking_poll_proceeds_on_a_live_device() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _ = queue;
    assert!(
        matches!(
            crate::render::context::poll_device_now(&device, &lost),
            crate::render::context::PollOutcome::Proceed
        ),
        "活设备上的非阻塞轮询必须继续"
    );
}

/// 指名等自己那次提交必须等得到：wgpu 在「等待的序号大于最后成功提交序号」时报
/// `WrongSubmissionIndex`（判成失败），故 `Queue::submit` 返回的序号必须可等。
/// 遮罩回读等的就是自己那次拷贝的序号（不是「最新一次提交」——那个目标会随渲染
/// 线程继续出帧而前移，回读可能一直等不到自己的结果）。
#[test]
fn polling_for_our_own_submission_completes() {
    let Some((_instance, _adapter, device, queue)) = create_test_device() else {
        panic!("requires GPU adapter but none available");
    };
    let lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Poll Own Submission Encoder"),
    });
    let submission_index = queue.submit(std::iter::once(encoder.finish()));
    assert!(
        matches!(
            crate::render::context::poll_device_wait(
                &device,
                &lost,
                &submission_index,
                std::time::Duration::from_secs(2),
            ),
            crate::render::context::PollOutcome::Proceed
        ),
        "指名等自己那次提交必须继续"
    );
}
