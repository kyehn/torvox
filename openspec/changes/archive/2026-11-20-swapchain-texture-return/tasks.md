# 任务

## 一、定位

- [x] 复现模拟器上的永久停帧：取纹理超时后画面停在旧帧，`transient render error` 持续累积
- [x] 核对 wgpu-hal 30.0.2 的 Vulkan 后端：未呈现即丢弃的交换链纹理，`discard_texture` 是空实现
- [x] 列出应用侧全部丢弃已取纹理的路径（尺寸不符、管线缺失、工作线程迟到结果）

## 二、实现

- [x] 新增 `release_acquired_texture`：清成背景色后呈现，归还交换链图像
- [x] 尺寸不符分支：先归还再 reconfigure
- [x] 管线缺失分支：归还后返回原错误
- [x] 工作线程：回送失败且带纹理时归还；`lane.finish()` 移到回送之后

## 三、验证

- [x] 回归测试 `returned_texture_is_cleared_to_the_background`（GPU 回读断言背景色）
- [x] `cargo test --workspace`：933 passed, 0 failed
- [x] `cargo clippy --all-targets -- --deny warnings`、`cargo fmt --check`：通过
- [x] 模拟器 release 构建：冷启动、输入、旋转、`wm size` 改尺寸后持续出帧，本应用 0 条 W/E
- [x] `scripts/check.nu` 全量门禁：通过
- [x] `openspec validate --all --strict`：通过
