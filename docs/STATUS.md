任何情况下禁止对本文件进行任何修改

---

## 未被解决的问题

基于92787a573601880a7d118eddc67d8fb1a1a27bdb提交

- 切换后台返回终端闪烁：首帧提交后曾立即撤掉开孔遮罩，但 `present` 只把缓冲排入
  队列，SurfaceFlinger 要到下一次 vsync 才锁存上屏，那一拍开孔只剩窗口底色。已改为
  把「已有缓冲区」的发布延后一个显示帧（`6c7c8ec9`），`false`（surface 被拆）仍立即
  生效；模拟器用例 `bufferPresentIsPublishedOneDisplayFrameLate` 通过，真机复验待做，
  复验前仍按未解决计。
