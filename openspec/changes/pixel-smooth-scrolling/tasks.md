## 1. 渲染侧像素偏移

- [x] 1.1 确认 cell 管线 Y 平移承载（读 `cell_builder.rs` 实例布局，定 uniform 还是实例顶点），输出结论到 design Q1
- [x] 1.2 实现视口 Y 像素偏移接收与应用（钳制 `[0, cellHeight)`，缺省 0），`cargo check -p native` 通过
- [x] 1.3 JNI 像素偏移入口（仿 `setScrollOffset` 守卫风格），host 单测覆盖钳制边界

## 2. Kotlin 余量同步

- [x] 2.1 `onScroll` 将 `scrollAccumulatorPx` 余量同步给渲染侧（复用 `forceRender` 节奏，无新线程；fling 保持整行步进），模拟器 logcat 确认余量逐帧到达（6-36px 连续值）
- [x] 2.2 松手/停 fling 时余量归零对齐 + 末帧请求，边界（顶/底）余量钳制为 0

## 3. 验证与门控

- [x] 3.1 maestro 慢速拖动位移连续性验证（截图/OCR），方向 OCR 复核（上滑看历史不变）
- [x] 3.2 Rust 帧计时同级确认（avg/p95 0ms 级），`cargo test -p native --lib` 无新增失败
- [x] 3.3 `cargo fmt --check`、`clippy` 无新增警告、`test-gradle.nu` 通过，`openspec validate --changes` 通过
