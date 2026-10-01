# 参考

- [termux-app](https://github.com/termux/termux-app)：文本选择，会话列表
  - 反例：用 `termux-exec` 做 `LD_PRELOAD` 重定向；其 CPU 逐格 `Canvas.drawText`
- [ghostty-android-terminal](https://github.com/sylirre/ghostty-android-terminal)：选择系统 UX（选择状态由模拟器拥有）。搜索覆盖层不改终端尺寸免 `SIGWINCH`。流畅滚动。
  - 反例：四槽字体设置。
- [termlib](https://github.com/connectbot/termlib)：`applyHandleDrag` 锚点语义加交叉翻转，拖动手柄越过静止手柄时归属权交换；选择含多行反向判定、URL 尾随标点修剪加括号配对计数。
  - 反例：普通/PASSWORD 双输入模式切换
- [zed-android-port](https://github.com/GeneralKaos666/zed-android-port)：前台进程组经 `tcgetpgrp` 获取、回退 shell 子进程并读 name/cwd/argv；kill 先 `killpg` 前台组再 `kill` shell。
- [wgpu-in-app](https://github.com/jinleili/wgpu-in-app)：
  - 反例：JNI 导出用 `jni_fn` 宏
- [zelland](https://github.com/njreid/zelland)：surface 就绪竞态用 `PENDING_SIZE` 独立存尺寸弥合初始化窗口。鼠标映射须用实时 cell 尺寸而非编译期常量。
- IME composing 增量 diff 同步，避免全量重设。
