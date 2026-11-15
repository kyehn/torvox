# 状态

## 未被解决的问题

（无）

## 已解决

### 输入法弹出时切后台返回终端大概率闪烁

- 现象：`logcat_2026-11-15_13-25-24.txt` 里的两次真实 HOME 往返各出现一次
  （`setRenderPaused: paused=true` → `read_cover_frame` → 约 26s 后台 → `attachWindow`
  新指针 → `warmup: presenting the retained last frame`）。
- 根因：切后台时运行期发布了开孔遮罩，但窗口不会自己重绘——`SurfaceView` 的绘制
  只由组合层的失效触发，而遮罩发布发生在渲染线程上，与组合层时机无关。结果遮罩
  只在平台**拆掉 surface 之后**才被画，那一帧没有任何窗口再看，屏幕上留的是离开前
  的空开孔，回前台首帧露出的正是它。
- 修复：发布遮罩即让 `SurfaceView` 重绘（`observeHoleCover`），且暂停期间即使 surface
  仍有缓冲也照画遮罩；撤下遮罩按交换链队列深度延后两帧
  （`desired_maximum_frame_latency = 2`）。开孔状态序列新增「已涂满」事实，用于区分
  「有遮罩」与「遮罩真的进了窗口」。
- 验证：`BackgroundResumeFlickerInstrumentedTest`（模拟器，真实 HOME 往返，含键盘在
  场形态）断言「遮罩在 surface 仍在时已涂满窗口」，并做过反向对照——去掉发布侧的
  重绘后该断言失败，即原实现确实漏了这一帧。
