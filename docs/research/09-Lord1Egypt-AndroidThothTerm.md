# 09 Lord1Egypt/AndroidThothTerm

上游：<https://github.com/Lord1Egypt/AndroidThothTerm>
本地：/tmp/competitor-study/Lord1Egypt__AndroidThothTerm
版本：主线 2026-10-07 取样，1561 文件。JackPal 血统 Java View 终端加多发行版 garden，无 Rust 无 GPU 渲染。核心约 40 个 Java 与 C 文件已通读，其余发行版 flavor 与资源已抽查。

## 架构

`emulatorview/` 为 JackPal 核心库：自研 VT 解析、环形回滚 transcript、View 手势选择、双线程会话、JNI fork 执行。`libtermexec/` 为 PTY 专属 JNI：fork、setsid、重定向、exec 标准流程。`term/` 与 `term-ubuntu/` 为应用壳：常驻服务、会话列表、窗口切换、虚拟键。`garden-*` 按发行版切 flavor：运行时、rootfs 管理、安装界面、构建脚本。

## 终端与渲染

自研 VT100 状态机，仅 CSI 与 SGR 子集。逐行经 `Bidi` 做逻辑视觉列映射后按样式段绘制，先背景后文字，光标单独处理。整屏同步逐行绘制，无脏矩形批处理。此条路线不可抄，本项目走 Ghostty 解析加 wgpu 渲染。

## 选择滚动与字体

选择状态机在视图层：长按取词、双手柄拖拽、经 transcript 提取文本。滚动为顶部行偏移加边界钳制，fling 手势支持。字体缩放为双指手势改字号后重建渲染器。交互状态机可参考，绘制实现不可抄。

## 会话与通知

会话构造即起读写双线程，结束关流清屏。服务持会话数组并监听标题，创建即进前台常驻通知。窗口列表即会话列表，每行标题加关闭按钮，点击切换。

## 发行版

运行时固定家目录与客户 shell，组 proot 参数；rootfs 管理下载校验后经 staging 解包替换。属内嵌 proot 加预装发行版，本项目禁止，仅保留原子替换与失败清理思想。

## 与本项目对照

解析、渲染、会话归属三条主线均不可抄。选择手柄交互、键栏布局、会话列表回调、常驻通知文案、staging 清理语义可参考行为。

## 可参考点

- P0：长按取词加拖拽手柄加文本提取的交互状态机；虚拟键布局；会话数组回调加标题监听；常驻通知做法；staging 加失败清理语义。
- P1：读写线程划分与结束释放顺序；resize 失败保护；双指缩放步长。
- P2：双向列映射测试思路；回滚与选择用例清单。

## 禁止项勿抄

自研解析器、Canvas 渲染管线、内嵌 proot 与预装发行版、桌面与远端服务、自定义主题、会话持久化误读。

证据：`TerminalEmulator.java:2039`、`TranscriptScreen.java:561`、`UnicodeTranscript.java:1101`、`EmulatorView.java:233-1914`、`TermSession.java:724`、`process.c:124-179`、`TermService.java:69-123`、`WindowListAdapter.java:82`、`ExtraKeysView.java:89`、`GardenRuntime.java:34`、`RootfsManager`。
