# 参考

- [termux-app](https://github.com/termux/termux-app)：文本选择系统（`TextSelectionCursorController` 锚定、手柄拖动状态机、宽字符吸附），`onGetContentRect` 等于列/行乘字体像素加 TopRow 滚动偏移、手柄 `PopupWindow` 加 300ms 防误关、宽字符吸附。`WcWidth` 按 Unicode 15 判定宽字符。`TextStyle` 64 位打包前景/背景/属性。选择文本按 wrap 感知拼接，不硬插换行；列转 char 经宽字符换算防切错 CJK
  - 反例：用 `termux-exec` 做 `LD_PRELOAD` 重定向；其 CPU 逐格 `Canvas.drawText`
- [ghostty-android-terminal](https://github.com/sylirre/ghostty-android-terminal)：选择系统 UX（选择状态由模拟器拥有、tapCount 多击、Callback2 加 onGetContentRect 菜单锚定、selectionGeometryKey、边缘滚动）。初始 winsize 带 `ws_xpixel`/`ypixel`。PTY 摄取 feed 前 NUL 剥离走 `memchr` 快路径。搜索覆盖层不改终端尺寸免 `SIGWINCH`，防抖 150ms，高亮复用选择机制
  - 反例：`TerminalFontStore` 四槽字体
- [Haven](https://github.com/GlassHaven/Haven)：`cursorKeyAppMode` 跟踪、alt 屏滑动转方向键时 SS3/CSI 区分（应用光标模式（DEC 私有模式 1）下方向键须编码为 SS3（`ESC O A`）而非 CSI（`ESC [ A`），否则 vim/less 误读）。Popup 内 `startActionMode(TYPE_FLOATING)` 静默 no-op
-[termlib](https://github.com/connectbot/termlib) ：`applyHandleDrag` 锚点语义加交叉翻转，拖动手柄越过静止手柄时归属权交换；选择含多行反向判定、`resize` 钳制、URL 尾随标点修剪加括号配对计数、`TerminalInputConnection` 组合键与 IME 显示控制
  - 反例：普通/PASSWORD 双输入模式切换
- [zed-android-port](https://github.com/Dylanmurzello/zed-android-port)：前台进程组经 `tcgetpgrp` 获取、回退 shell 子进程并读 name/cwd/argv；kill 先 `killpg` 前台组再 kill shell。
- [wgpu-in-app](https://github.com/jinleili/wgpu-in-app)：`acquire` 五分支处理（Success/Suboptimal 直用，Timeout/Outdated/Lost 重配重试，Occluded 返 None）与 Android `view_formats` 取单格式；`ANativeWindow` 引用计数 RAII；零尺寸 clamp；`resize` 幂等判定。 JNI 会话句柄用全局注册表，不用 `Box::into_raw` 裸指针
  - 反例：JNI 导出用 `jni_fn` 宏
- ［zelland］（https://github.com/njreid/zelland）surface 就绪竞态用 PENDING_SIZE 独立存尺寸弥合初始化窗口。鼠标映射须用实时 cell 尺寸而非编译期常量
- [termux-kotlin-app)](https://github.com/reapercanuk39/termux-kotlin-app) 字符串转 argv 用 BSD 四态机 `ArgumentTokenizer` 安全拆分。搜索高亮覆盖层对照搜索反色/定位高亮
- ［console］（https://gitlab.gnome.org/GNOME/console）收窄搜索用 contains 而非 startsWith，保持当前匹配；Copy 无选择时置灰而非隐藏

## PTY 与进程
- fork 前预构建 CStrings、child 只做 AS-safe 调用
- spawn 前先应用初始 winsize（含像素字段），否则首帧竞态致折行错位；

## 输入与 IME

- IME composing 增量 diff 同步，避免全量重设。
- `InputEvent` 触摸状态机与多会话委托函数集为 host 可测纯逻辑

## 文本与字体

- CJK/emoji 分类边界测试对照字体分类，防区间过度扩张致 tofu

