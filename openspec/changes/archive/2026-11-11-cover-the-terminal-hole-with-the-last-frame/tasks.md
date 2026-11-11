# 任务

- [x] 撤销 `setSurfaceLifecycle(SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT)`（实测证伪）
- [x] 原生侧：帧累加器的像素回读 + JNI 导出
- [x] 运行期：渲染线程停下时回读并发布遮罩，新帧到达即撤
- [x] 组合层：「还没有缓冲区」时画遮罩，没有遮罩才退回平底色
- [x] 测试：原生侧回读单测（GPU）+ 仪器化用例（遮罩发布、画面非单色、恢复后撤下）
- [x] 更正 `render-stability` 里与实测相反的断言，并补上遮罩的要求与场景
- [x] 定向校验、归档变更、同步主规约
