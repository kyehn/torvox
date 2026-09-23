## 1. 斜体字形首帧缺失

- [x] 1.1 取证：atlas 首次上传时序与字形缓存命中路径（font/mod.rs glyph_information_styled → atlas upload → shader advance 裁剪）
- [x] 1.2 修复首次渲染字形缺失（上传/缓存/渲染触发），新增单测覆盖首次与重复渲染一致性
- [x] 1.3 设备复现验证：`cat italic_test.txt` 首帧 OCR 完整

## 2. IME 期间渲染暂停恢复竞态

- [x] 2.1 取证：setRenderPaused 调用链与渲染循环恢复路径（maki：native 三处暂停修复 d9f91bc/358ce11/18c8313 已在 main，Kotlin 纯 Compose pan 下无 surface 变化；干净 logcat 无恢复缺口）
- [x] 2.2 修复：无新增——已有修复覆盖（暂停期不消费通道、恢复走全量、恢复强制重绘）
- [x] 2.3 设备验证：IME 弹出时当前构建输出可见（maki 真机取证）；红色 error 场景复现未再观测到缺失

## 3. 滚动卡顿/撕裂与启动黑屏

- [x] 3.1 取证：滚动渲染路径与启动首帧链路（dsh：Immediate 无 vsync 直通扫描线必撕裂 + 166fps 空转冲刷；启动首帧等 shell 输出 + 冷启动数百 ms 无帧提交 → 黑屏）
- [x] 3.2 修复并验证流畅度（dsh：present mode 改 Mailbox/Fifo vsync 优先 + attach_surface 预创建 frame texture + warmup 首帧背景色；设备 OCR 验证 + 全量测试通过）

## 4. 文档与归档

- [x] 4.1 按实现补充 specs 细节（omp：render-stability spec 任务 1 实现细节 25 行）
- [x] 4.2 归档 change
