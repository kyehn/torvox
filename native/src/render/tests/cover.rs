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
