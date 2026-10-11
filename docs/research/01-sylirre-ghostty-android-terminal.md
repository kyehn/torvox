# 01 sylirre/ghostty-android-terminal

上游：<https://github.com/sylirre/ghostty-android-terminal>
本地：/tmp/competitor-study/sylirre__ghostty-android-terminal
版本：主线 2026-08-20 取样，186 文件，Java 66、XML 49、头文件 34、C 5，Java/C 约 4.1 万行。全部文件已通读。

## 架构

Java 拥有会话与渲染，C 仅做胶水，VT 全委托预编译 `libghostty-vt.a`。包 `io.github.sylirre.terminal`，无 appcompat/material，全自定义 View。`term/` 管会话与模拟器，`ui/` 管渲染与交互，`native/` 管 PTY 与 Ghostty C API 胶水，`native/arm64chroot|arm64emu|chroot-ng` 管用户态发行版，`native/ghostty-vt/prebuilt` 提交双 ABI 静态库。

关键路径：`TerminalSession.readLoop` 读 PTY → `terminal_jni.c` 内 `vt_write` 同步回调（write-pty/bell/title）缓冲于 `TermCtx.out/events` 同次 JNI 返回，零上行调用。`TerminalEmulator` 全方法 `synchronized` 串行化非线程安全 Ghostty。`TerminalView.onDraw` 拉最新快照，同属性连续 cell 合并 `drawText`，reader 线程合并失效丢中间帧。

## 与本项目对照

本项目 Rust 拥有 PTY 与 Ghostty 并经 wgpu Vulkan 渲染，Kotlin 仅收轻量事件；竞品 Java 拥有 fd 与线程并经 Canvas 拉快照过 JNI。本项目 `libghostty-vt-sys` 跟 master 无补丁；竞品 pin 提交 `.a`。字体本项目读系统 `fonts.xml` 缓存、单主字体、`.termux/font.ttf|fonts/`，不复制；竞品 SAF 导入复制四字重。主题本项目构建时拉 alacritty-theme、固定 10 名、dracula_plus 默认；竞品 19 色展 256 加 JSON 自定义编辑器。搜索本项目 `regex` 字面禁正则；竞品 native 全缓冲扫复用 selection 槽。选择/键栏本项目 Termux 长按选禁多击、方块不闪、固定 2x7；竞品支持双三击、闪烁、ExtraKeys 多 profile 可编辑。

## 可参考点

- P0：回调缓冲同次返回（`terminal_jni.c:100-119`）对应 Rust feed 一次返回 reply 与事件；单锁串行 Ghostty（`TerminalEmulator.java:23-40`）；布局后 spawn 并跳过 no-op resize 防 SIGWINCH 清空；键编码前同步终端模式；粘滞键简化无双击锁定；IME 行编辑态切换重启。
- P1：搜索单锁扫描加高亮加 dirty 免重扫加环形上限加 BMP case-fold（改用 `regex` 字面实现）；选择终端持有自跟随加拖柄跨端翻转加边缘滚动；winsize 像素加 XTWINOPS 回复；OSC8 链接二次确认；备份原子暂存改名与恶意 tar 约束。
- P2：复合键标签与防 tofu 字形替换思路（须用系统或图标包实现）；网格尺寸 HUD；主题推送全会话时机；真机测试矩阵。

## 禁止项勿抄

内嵌用户态或虚拟机（arm64chroot/chroot-ng/arm64emu 与 rootfs/ISO 资产）、背景图与透明模糊、自定义主题编辑器、键栏布局编辑器、光标样式与闪烁与回滚设置，以上均撞 PROHIBITED。另禁 CPU/OpenGL 回退、沉重快照过 FFI、Java 持会话、pin 库加本地补丁、硬编码主题色、复制字体、自定义环境变量透传。

证据：`docs/architecture.md:1-646`、`TerminalEmulator.java:6-297`、`TerminalSession.java:18-465`、`SessionManager.java:12-151`、`SessionService.java:20-216`、`TerminalView.java:57-2506`、`terminal_jni.c:29-119`、`pty_jni.c:1-17`、`ExtraKeysConfig.java:27-699`、`SearchBarView.java:30-299`、`TerminalFontStore.java:16-108`、`ThemeStore.java:35-177`。
