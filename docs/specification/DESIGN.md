# 设计

## 依赖

- 依赖/工具尽量使用最新版本，尽量不固定小版本
- 未声明的细节 参考 <https://github.com/termux/termux-app> 和 <https://github.com/sylirre/ghostty-android-terminal>

## 习惯

- 只允许设置下面出现的环境变量，不允许未声明情况，尽量不要读取环境变量

- 尽量选择简洁 可靠 优雅 先进 激进 不妥协的设计方案

- 下面如“有 a b c 项”指有且只有，不可有未声明行为。

- 所有异常处理代码必须最少出现并且最简处理，极端情况/错误 -> 输出日志并崩溃退出；设置数据错误 -> 清除设置数据。所有 Fallback 必须严格限制，只允许下面声明的回退。

- 最小体积，不做任何多余或不必要功能，能不做则无必要。

- 高性能、低功耗是目标，性能优先，低内存友好

- 不做过多冗余，减少兜底，尽早抛出错误，避免浪费资源。

- 不做任何未要求的 Fallback 机制，输出日志并崩溃退出不要掩盖错误。

- 使用通用规范，最大限度使用外部依赖，最低限度自定义实现。

- 图标（非软件桌面图标）使用 系统提供 或 图标包，不 vendor，不从外部单独下载

- 应该适配 Android 软件启动屏/动画

- 解决问题前必须通过调试确定原因，修复后需要验证，不做猜测

- 较低安全性/隐私策略，方便快捷更重要，不设计权限管理

- 简洁设计，不干涉用户数据

- 遵循 Material Design 3 最新标准

## 架构

### Rust

- Rust crate 结构：

  ```text
  root workspace/
  ├── Cargo.toml
  ├── native/
  │   ├── Cargo.toml
  │   └── src/
  │       ├── *.rs
  │       ├── android/  ← JNI 导出（ffi.rs）
  │       ├── render/
  │       └── terminal/
  ```

- 以 Ghostty 作为终端状态的单一来源（Single Source of Terminal State），不重复实现 Ghostty 已有功能

- 渲染完全在 Rust 侧通过 `wgpu` 完成。Kotlin 仅通过直接 JNI 接收轻量事件，无沉重网格数据跨越 FFI 边界。

- 每个终端会话独占一个线程，产出扁平化的单元格数组；共享渲染线程消费这些数组并驱动 `wgpu`。

- 会话归属于 Rust，而非 Kotlin。PTY 的 `fork` / `exec`、Ghostty 终端与渲染循环均由 Rust 管理。Android Activity 生命周期要求应用正确处理以下场景：

  - **Activity 重建**（屏幕旋转、配置变更）：Surface 被销毁并重建，旧 `ANativeWindow` 指针失效。
  - **进程被回收**：系统可能终止应用进程，全部 Rust 状态与 PTY 进程随之消失。
  - **后台切回前台**：应用须在不销毁终端状态的前提下恢复渲染。

- GPU Vulkan 渲染（无 CPU / OpenGL 回退）。

- 上游 `libghostty-vt` / `libghostty-vt-sys` 固定跟踪 git master，无本地补丁。

- 剪贴板集成：通过终端序列（OSC 52）与用户交互读写系统剪贴板。仅在 ghostty 支持时实现，不做过度复杂工作。

- 支持下划线颜色 (SGR 58) 和上划线 (SGR 53)

- libghostty-vt 使用参考 <https://github.com/sylirre/ghostty-android-terminal/blob/main/docs/architecture.md#libghostty-vt>

### Kotlin

- 日志必须在 Android `logcat` 中可见以便调试，同时避免在渲染热路径上产生性能开销。不写入文件，不保存日志

- `applicationId = "com.termux"`。

- 包名 `terminal.emulator`。

- 使用 AOSP testkey（`android/app/aosp-testkey.p12`）签名，禁止 debug 签名，禁止使用其他签名

## 设置

- **字体大小**：提供调节条。默认大小与可选范围须参考常见设备分辨率（以 Termux 为基准），并结合设备实际分辨率进行限制。

- **字体选择**：支持从字体列表中选择字体，显示当前设置的主字体（即使用户未设置，`fonts.xml` 会提供需要的信息）
  - 遵循 Android 系统 `fonts.xml`，从 `fonts.xml` 解析出所有需要的信息转换到需要的格式，如果用户选择了某字体需要对对应的信息替换。
  - 系统不存在 `fonts.xml` 或其内容无法解析，软件输出日志并崩溃退出，不做复杂处理。
  - 相关设置出现错误时（如默认字体设置错误）重置应用数据
  - 从不 复制/移动 文件
  - /data/data/com.termux/files/home/.termux/font.ttf 如果存在设置为默认字体
  - 需要支持 多字重字体 动态字体 文件
  - 如果是文件夹 /data/data/com.termux/files/home/.termux/font 存在加入字体扫描路径并显示其中字体在字体列表。
  - 只有一项主字体选择，不支持 粗体/斜体 单独设置，粗体/斜体 跟随主字体
  - 字体列表内不得重复，“DroidSans”和“Droid Sans”以及“Droid Sans Regular”重复，不得做手动判断，而是要求外部库 api 提供正确的字体列表
  - 字体列表中不展示“系统默认”等含糊选项，不显示不存在的字体，不使用任何硬编码

- **实际字体信息框**：展示字体的实际使用情况，包括 主字体、其他字体（除主字体外当前被使用的字体的列表）、字体实际大小与单元格信息。

- **软件主题**：“日间”“夜间”“跟随系统”三种。

- **终端主题**：作用于终端页面与修饰键栏，默认“Dracula Plus”主题，开启 “跟随系统”开关后支持设置日 / 夜两种终端主题间，并跟随“软件主题”在日 / 夜两种终端主题间切换。
  - 默认主题在第一次使用时即被应用
  - 主题效果支持预览，主题名称不在预览框内而是下面，长主题名称需要可以被正常显示

- **修饰键栏**：
  - 不能和系统全面屏手势冲突，包括“底部上滑”绝不能触发按键
  - CTRL ALT 等键能正常工作
  - 所有按钮正确接线，动画/逻辑和 termux 实现基本一致，动画简短 不复杂 不卡顿 快速 不浪费时间
  - 布局/按键和 termux 完全相同
  - 修饰键栏支持向左滑动进入 文本输入框（参考 termux），位置：修饰键栏 文本输入框
  - 固定2行7列（高度 宽度 等均参考 termux）。

- **Shell 启动入口路径及参数设置框**。提供保存按钮，支持保存和显示设置的文本，未设置时为空。保存时不检查文斌，不检查路径是否存在，不检查参数是否合法。支持 `/data/data/com.termux/files/usr/bin/sh` `/system/bin/sh /data/data/com.termux/files/usr/bin/login.sh` `/data/data/com.termux/files/usr/bin/bash -l`。不支持 `/data/data/com.termux/files/usr/bin/login.sh`，即启动入口应该是二进制文件且必须是可绝对路径。不对文本进行检查，不检查路径/参数是否正确，不进行特殊处理。

- **Bootstrap**：支持 HTTP(S) URL 与本地文件安装。
  - 只提供 Termux 预设选项，使用 apt-android-7（较大值） 和 2026.02.12-r1（最新值），不提供 apt-android-5 2022.04.28-r6 等旧值，从 termux-app/app/build.gradle 提取逻辑
  - 原子化替换 /data/data/com.termux/files/usr/ 目录，（安装时 原 usr 重命名为 usr.xxxxx（随机后缀），安装完成后旧目录由用户手动删除，不自动删除）
  - 不记录 Bootstrap 状态，不得生成安装标记
  - 不得特殊化设置权限，按照 termux 同款流程设置，不额外设置某些目录
  - 只允许设置下面的环境变量：
    - `HOME` 和 `TERMUX_HOME_DIR_PATH` 为 /data/data/com.termux/files/home
    - `PREFIX` 和 `TERMUX_PREFIX_DIR_PATH` 为 /data/data/com.termux/files/usr
    - `TMPDIR` 和 `TERMUX_TMP_PREFIX_DIR_PATH` 为 /data/data/com.termux/files/usr/tmp
    - `LANG` 为 `en_US.UTF-8`
    - `COLORTERM` 为 `truecolor`
    - `TERM` 为 `xterm-256color`
    - `TERMUX_VERSION` 为 0.119.0-beta.3
  - 不得设置 `LD_LIBRARY_PATH` `PWD` `LD_PRELOAD`
  - 宿主透传变量，仅宿主存在时透传，不硬编：`ANDROID_ASSETS`、`ANDROID_DATA`、`ANDROID_ROOT`、`ANDROID_STORAGE`、`EXTERNAL_STORAGE`、`ASEC_MOUNTPOINT`、`LOOP_MOUNTPOINT`、`ANDROID_RUNTIME_ROOT`、`ANDROID_ART_ROOT`、`ANDROID_I18N_ROOT`、`ANDROID_TZDATA_ROOT`、`BOOTCLASSPATH`、`DEX2OATBOOTCLASSPATH`、`SYSTEMSERVERCLASSPATH`。
  - 必须兼容 nix-on-droid，nix-on-droid 需要提供和 termux bootstrap 一致的格式，软件不做任何特殊兼容。测试：下载 <https://github.com/kyehn/nix-on-droid/releases/download/bootstrap-unstable/bootstrap-x86_64.zip> 或从源码编译，通过 bootstrap 安装逻辑（不得直接解压/复制），使用终端输入 nix build 命令（不得使用adb shell 替代）进行测试。
  - 禁止对 nix-on-droid 特殊处理，termux/nix-on-droid bootstrap 共用安装逻辑代码，postinstall 只在存在时运行，不做无意义检查/校验，出现问题正常报错就是。

- **清除应用数据按钮**，清除与 /data/data/com.termux/files 无关的设置数据/缓存数据等。应用数据与用户数据为不同概念，除 Bootstrap 设置外不得修改用户数据（即 /data/data/com.termux/files 目录）。

## 终端

### 终端页面

- 脏跟踪，跳过干净快照，跳过逐行复制，减少突发输出期间的工作量

- 内容横向溢出到右侧时，修饰键栏的向右按键（->）将可见区域向右移动，修饰键栏的向左按键将可见区域向左移动，移动范围不超过内容

- 部分接口支持批量查询以保证性能

- 只渲染当前使用的会话，后台会话/切换应用/进入设置时暂停渲染

- 输入光标为方块样式（高度 宽度 等均参考 termux），不闪烁

- 切换应用返回或从应用设置返回终端应该正常渲染且无 进入卡顿 黑屏 闪烁 跳跃。输入弹出/隐藏时无卡顿 闪烁 跳跃 压扁 拉伸 溢出

- 支持 CJK，能正常处理，比如退格一次一个汉字而不是两次一个汉字。

- CJK 字体应该被正常渲染且和设置的字体对应而不是其他字体，如对于简体中文用户通常使用 Noto Sans CJK SC 而不是 Noto Serif 或 Noto Sans CJK JP，CJK 字体渲染速度应该和西文字体基本一致。

- 退格应该流畅，渲染不应卡顿

- **文本选择**：应该和 termux 设计一致，终端支持长按文本选择，被长按文本单元格反色，文本左右侧出现可拖动控制柄（可灵活拖动，流畅不卡顿，拖动时菜单隐藏），文本附近显示选项菜单（菜单始终不遮挡被选择文本，如果长按的是无内容区域：粘贴。如果是有内容区域：复制 分享 全选 打开链接（OSC 8 超链接）/打开文件（根据内容选择是否显示））
  - 控制柄/菜单等样式遵循 termux 设计和系统样式
  - 合理使用 GHOSTTY_TERMINAL_OPT_SELECTION 及其他 api
  - 全选后复制功能必须能够正常工作，全选只涉及有内容区域。
  - 弹出菜单始终不遮挡被选择文本，必须保持合适距离，包括变更选择范围后（参考 termux 实现），按钮必须可直接点击而不是两次。
  - 打开链接/打开文件 只在选择文本长度合理无空格时，只检查 链接/文件位置 是否格式匹配，不检查 链接/文件 的实际可用性，点击后通过系统 api 进行跳转
  - 打开文件 点击后检查文件是否实际存在，其他应用可以编辑和回写。

- 支持全功能输入法（不限制输入法特性），支持 CJK 输入法

- 支持 连字 kitty 图像协议 等特性（参考 ghostty-android-terminal 实现）

- 支持鼠标操作（参考 ghostty-android-terminal 实现）

- 支持按像素流畅滚动（参考 ghostty-android-terminal 实现）

### Shell

- Shell 启动入口为空时依次尝试 /data/data/com.termux/files/usr/bin/bash 和 /data/data/com.termux/files/usr/bin/login，无其他任何回退。

- 默认 LANG 为 en_US.UTF-8

- shell 崩溃（非主动正常退出）保留现场不关闭会话（参考termux，如执行 exit -1 后输出 [Process completed (code 255) - press Enter]，不主动关闭会话），正常退出时（如 exit 命令或用户点击关闭按钮）关闭会话

- 启动入口失败不得 Fallback，保留输出显示（参考 termux）

- 回滚行数和 termux 保持一致，如 2K

### 修饰键栏

- 修饰键栏默认布局跟随 Termux（基本一致），支持左滑与右滑（参考 Termux）。

- 修饰键不应该和全面屏手势冲突，不应该被上滑手势触发

- 修饰键动画应该较快，反应轻快

- 粘滞键可以被正常使用并且动画正常

- 修饰键栏不被输入法遮挡，在输入法弹出/隐藏时跟随移动

- 修饰键栏使用和终端相同的配色。

- **文本搜索输入框**：当文本搜索时，文本搜索输入框取代修饰键栏位置，具有 文本输入框 大小写匹配 当前顺序/总匹配数 上一个 下一个 关闭 等按钮
  - 匹配文本的长度/搜索频率需要被限制
  - 搜索可滚动显示的区域而不只是当前屏幕
  - 匹配到的单元格反色
  - 被 上一个 下一个 定位匹配的单元格特殊高亮

## 侧边面板（合理布局，不可溢出）

- **会话列表**。
  - 每一项包括 “会话序号 目录路径”（点击切换会话，目录路径可能需要缩写，实现参考 termux），关闭按钮（或支持向右滑动进行关闭）
  - 会话序号从1开始递增，列表改变时也是如此。
  - 需要实现 工作目录跟踪 (OSC 7/9/1337)

- 添加会话按钮。
- 重置终端 按钮，通过 ghostty_terminal_reset 重置 ghostty terminal 状态以恢复卡住的终端，清除滚动条
- 文本搜索按钮。
- 显示 / 隐藏输入法按钮。
- 设置按钮。

## 软件

- 应用启动时检查应用数据兼容性，若存在问题可清除应用数据以确保正常启动。

## 禁止实现，如果存在相关代码或文档需要彻底清理，不得存在任何相关代码

- 选中菜单中的 ◀ / ▶ 锚点移动项：左右控制柄应该可以直接拖动。
- Bootstrap zip 的 sha256 sidecar 校验。
- 自定义环境变量：不通过环境变量接收用户设置或在内部传递数据。
- 会话数据持久化 / 恢复。
- 粘贴确认对话框。
- 实体键盘快捷键设置。
- Model Context Protocol
- termux-api
- 环境变量编辑功能
- 背景图片，透明背景，背景模糊
- termux.env 文件及相关逻辑代码
- 内嵌 proot
- 内嵌 bootstrap，预装发行版
- 桌面环境，X11

### 横向/平板

- **固定/取消固定 按钮**：在横向/平板模式允许将侧边面板固定显示，终端页面和修饰键栏减小空间

### 设置

- **光标闪烁开关**。

- **光标闪烁速度**：提供调节条，范围与精度须受限。

- **光标样式**：方块、竖线、下划线。

- **Shell 启动入口状态**。

- **自定义终端启动目录**。

- **终端回滚行数**：提供调节条，范围与精度须受限。

- **修饰键栏布局编辑器**：可以对修饰键栏的布局进行修改（参考 ghostty-android-terminal）。
  - 不复杂设计，修饰键栏支持的按键种类固定（包括各种常用修饰键）。
  - 默认布局和 termux 完全相同（左侧修饰键栏需要原创）
  - 修饰键栏支持左右滑动切换多个布局，可以新增/删除/排序，默认：次修饰键栏 主修饰键栏 文本输入框
  - 编辑器支持预览和长按拖动位置，拖动位置为按键起始坐标
  - 提供删除和调整按键大小（如 1*2 或 2*2 以及其他可接受长宽）按钮
  - 提供“重置为默认”按钮。

- **自定义终端主题**：支持用户自定义主题：自定义主题可以修改（支持预览）和删除（未使用状态下，包括名称也可修改）。

- **Shizuku 集成开关**。支持从 <https://github.com/rikkaapps/shizuku> 获取权限并提供给 Shell，只为启动入口设置。新会话启动时检查权限如果已打开开关但未被实际授权显示对话框提示提供两个选项：关闭 Shizuku 集成开关/关闭会话（无其他会话时应用退出），Shizuku 需要 `adb shell /data/app/~~Sa3_liMwmjUIoWwNMF_x7w==/moe.shizuku.privileged.api-No2vLGXjkKhlYU6TcXtuHg==/lib/arm64/libshizuku.so` 类似命令激活

## 注意

- mksh 会在收到 SIGWINCH 信号时清除提示符。会话应该在完成首次布局后才生成 MainActivity，并且 resize 跳过空操作的调整大小。不要在会话生成时重新执行调整大小的操作。
