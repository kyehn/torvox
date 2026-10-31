# 设计

- 文中“有 `a` `b` `c` 项”的描述指有且只有，不得增加未声明行为。
- 未声明的实现细节可参考 [termux-app](https://github.com/termux/termux-app/tree/github-releases/v0.119.0-beta.3) 与 [ghostty-android-terminal](https://github.com/sylirre/ghostty-android-terminal) 同时遵循最佳实践。

---

## 依赖

- 依赖、工具尽量使用最新版本，尽量不固定小版本，版本声明尽量不写精确版本（尽量不用 `=x.y.z`）
- 多使用库，尽量避免自定义实现。
- 图标（非软件桌面图标）使用系统提供或图标包，不做 `vendor`，不从外部单独下载。

## 习惯

- 只允许设置下面出现的环境变量，尽量不要读取环境变量。
- 尽量选择简洁、可靠、优雅、先进、激进、不妥协的设计方案，追求质量而不是编写代码的速度
- 异常处理代码尽量简洁，不随意回退。通常：极端情况/错误 → 输出错误日志并崩溃退出；设置数据错误 → 清除设置数据。
- 不做不必要功能。
- 高性能、低功耗，低内存友好。
- 不做过多冗余，减少兜底，不做未要求的 `Fallback` 机制，尽早抛出错误，不要掩盖错误，避免浪费资源。
- 解决问题前必须通过调试确定原因，修复后需要验证，不做猜测。
- 不设计应用内权限管理或授权流程，不引入额外隐私策略 UI。

## 架构

### Rust

- Rust crate 结构：

  ```text
  root/
  ├── native/
  │   ├── Cargo.toml
  │   └── src/
  │       ├── *.rs
  │       ├── android/  ← JNI 导出（ffi.rs）
  │       ├── render/
  │       └── terminal/
  ```

- 不重复实现 Ghostty 已有功能。

- 渲染在 Rust 侧通过 `wgpu` 完成；Kotlin 通过直接 JNI 调用渲染与查询接口，只接收轻量事件与查询结果，不持有逐帧网格快照。

- 终端状态、PTY、VT 线程与 GPU 绘制归属 Rust；Kotlin 持有会话表（id、标题、界面状态）、设置与渲染线程调度，按 `sessionId` 寻址。

- 每个会话的 PTY 读取线程把输出送入有界通道，Ghostty VT 线程解析并维护终端状态；Kotlin 为当前活动会话运行一个渲染线程，经 JNI 调用 Rust 将当前帧绘制到 `ANativeWindow`。切换会话时停止旧渲染线程并启动新线程。

- Android Activity 生命周期要求应用正确处理以下场景：
  - Activity 重建（屏幕旋转、配置变更）：Surface 被销毁并重建，旧 `ANativeWindow` 指针失效。
  - 进程被回收：系统可能终止应用进程，全部 Rust 状态与 PTY 进程随之消失。
  - 后台切回前台：后台时暂停渲染，切回前台恢复渲染。

- GPU Vulkan 渲染（无 CPU/OpenGL 回退）。

- `libghostty-vt` / `libghostty-vt-sys` 跟踪 git master rev，无本地补丁。

### Kotlin

- 日志需要在 Android `logcat` 中可见以便调试，同时避免在渲染热路径上产生性能开销。不将日志写入文件，不保存日志。

- `applicationId = "com.termux"`。

- 包名 `terminal.emulator`。

- 使用 AOSP testkey（`android/app/aosp-testkey.p12`）签名，禁止 debug 签名，禁止使用其他签名。

## 设置

- **字体大小**：提供调节条，支持双指捏合缩放。默认字号按屏宽自适应，调节条的步长和范围必须合理并且是实际可设置数值，即渲染情况应该和设置 UI 一致。调节条与捏合缩放共用同一上界。
- **字体选择**：显示当前设置的主字体（默认使用 `/system/etc/fonts.xml` 的 monospace 字体），支持从字体列表中选择字体。
  - 完整字体列表只在实际显示字体列表时获取，字体列表需要被缓存直到应用关闭
  - 系统不存在 `fonts.xml` 或其内容无法解析，软件输出日志并崩溃退出，不做复杂处理。
  - 相关设置出现错误时（如默认字体设置错误）重置设置数据。
  - 从不复制/移动文件。
  - `/data/data/com.termux/files/home/.termux/font.ttf` 或 `/data/data/com.termux/files/home/.config/termux/font.ttf`（也支持 ttc otf 等格式）如果存在设置为主字体。
  - 支持多字重字体、动态字体文件。
  - 如果文件夹 `/data/data/com.termux/files/home/.termux/fonts` 或 `/data/data/com.termux/files/home/.config/termux/fonts` 存在则加入字体扫描路径并显示其中字体在字体列表。
  - 只有一项主字体选择，不支持粗体/斜体单独设置，粗体/斜体跟随主字体。
  - 字体列表无重复项，“DroidSans”和“Droid Sans”以及“Droid Sans Regular”重复，要求外部库 API 提供正确的字体列表。
  - 字体列表中不展示“系统默认”等含糊选项，不显示不存在的字体，不使用硬编码，字体列表不提供“从文件加载”选项。
- **实际字体信息框**：展示字体的实际使用情况，包括主字体、其他被使用字体（除主字体外当前被使用的字体的列表，包括符号字体和区域字体等）、字体实际大小与单元格信息等。
  - 对于区域字体情况应正确提示包括“非匹配区域”、“主字体已覆盖”、“未匹配到”等情况
- **软件主题**：“日间”、“夜间”、“跟随系统”三种。
- **终端主题**：作用于终端页面与修饰键栏，默认 dracula_plus 主题，开启“日夜切换”开关后可以分别设置日/夜两种终端主题，并跟随“软件主题”在两者间切换。
  - 构建时从 <https://github.com/alacritty/alacritty-theme> 读取主题配置，仓库不硬编码颜色，`catppuccin_mocha` 等主题名称在代码中应该只出现一次，硬编码主题列表 dracula_plus catppuccin_latte catppuccin_mocha monokai gruvbox_light gruvbox_dark tomorrow tomorrow_night tokyo_night tokyo_night_light github_dark_high_contrast moonfly pastel_dark，不得转换名称，不使用 Dracula Plus 命名
  - 默认主题在第一次使用时即被应用。
  - 主题效果支持预览，主题名称不在预览框内，而是在其下方，长主题名称可以被正常显示。
- **Shell 启动入口路径及参数设置框**。提供保存按钮，支持保存和显示设置的文本，未设置时为空。保存时不检查文本，不检查路径是否存在，不检查参数是否合法。
  - 支持如 `/data/data/com.termux/files/usr/bin/sh` `/system/bin/sh /data/data/com.termux/files/usr/bin/login.sh` `/data/data/com.termux/files/usr/bin/bash -l` 等写法。
  - 不支持相对路径如 `login.sh`（须为绝对路径），不支持无解释器直接执行脚本路径如 `/data/data/com.termux/files/usr/bin/login.sh`（须经 `/system/bin/sh` 间接执行）
  - 启动入口应该是二进制文件且是绝对路径。
  - 启动目录默认 `/data/data/com.termux/files/home`；可通过 `termux.properties` 的 `default-working-directory` 覆盖，非法值回退默认；不提供 GUI 设置。
- **Bootstrap**：支持 URL 与本地文件安装。
  - 提供 Termux 预设选项，不提供旧版本如 `apt-android-5 2022.04.28-r6`，从 `termux-app/app/build.gradle` 提取逻辑。
  - 原子化替换 `/data/data/com.termux/files/usr/` 目录，下载/安装前要求中间目录 `/data/data/com.termux/files/usr-staging` 不存在，如果存在则拒绝执行，原 `usr` 重命名为 `usr.xxxxx`（随机后缀），安装完成后旧目录由用户手动删除，不自动删除。
  - 不记录 Bootstrap 状态，不得生成安装标记。
  - 不得特殊化设置权限，按照 Termux 同款流程设置，不额外设置。
  - 禁止对其他 bootstrap 特殊处理，`postinstall` 只在存在时运行，不做无意义检查/校验，出现问题正常报错。
- **清除应用数据按钮**，清除设置数据/缓存数据等。不得影响 `/data/data/com.termux/files` 目录

## 终端

### 终端页面

- 脏跟踪，跳过干净快照，跳过逐行复制，减少突发输出期间的工作量。
- 对于 `nix --help` 这类命令，修饰键栏的向右按键（`->`）和向左按键（`<-`）以及向上和向下按键应该正确发送信号
- 部分接口支持批量查询以保证性能。
- 只渲染当前使用的会话，后台会话/切换应用/进入设置时暂停渲染。
- 输入光标默认方块样式（高度、宽度等均参考 Termux），可通过 `termux.properties` 的 `terminal-cursor-style` 设为 `block`/`bar`/`underline`，不闪烁。
- 切换应用返回或从应用设置返回终端应该正常渲染且无进入卡顿、黑屏、闪烁、跳跃。输入弹出/隐藏时无卡顿、闪烁、跳跃、压扁、拉伸、溢出。
- 只加载实际需要的字体（如 `DroidSansMono` `NotoColorEmoji` `Noto Sans CJK SC`），加载字体族如 `Noto Sans CJK SC` 而不是仅 `Noto Sans CJK SC Regular`，不加载未使用字体。支持非等宽字体作为主字体。
- 支持 CJK 字体，比如退格一次一个汉字而不是两次一个汉字。CJK 字体渲染速度应该和英文字体基本一致，退格 CJK 字符的速度应该和英文基本一致，不卡顿。
- **文本选择**：应该和 Termux 设计一致，终端支持长按文本选择，被长按文本单元格反色，文本左右侧出现可拖动控制柄（可灵活拖动，流畅不卡顿，拖动时菜单隐藏），文本附近显示选项菜单：
  - 长按有内容区域：复制、分享、全选；无可复制文本时不显示复制/分享；选区起点为 OSC 8 超链接时增加打开链接。
  - 长按无内容区域：仅反色该单元格；剪贴板有文本时显示粘贴，无文本时不显示菜单。
  - 控制柄/菜单等样式遵循 Termux 设计和系统样式。
  - 选区随滚动跟随，支持宽字符吸附 交叉交换归属 拖动时隐藏菜单 下缘拖动逐行滚动
  - 上下滑动终端时保持选区状态
  - 选区行间无空隙
  - 全选时尾部空行不入选
  - 不得支持 双击 三击 多击选择
  - 全选后复制功能必须能够正常工作，全选只涉及有内容区域。
  - 弹出菜单尽量不遮挡被选择文本，保持合理位置，包括变更选择范围后，按钮一次点击即生效无需重复点击，参考 Termux 实现
  - 链接识别仅查询 `libghostty-vt` 的 OSC 8 超链接，不扫描纯文本裸 URL，不检查链接的实际可用性，点击后交系统 API 跳转。
- 支持全功能输入法，不限制输入法特性
- 支持单码位整形与复杂文种叠加、kitty 图像协议等特性（参考 ghostty-android-terminal 实现），不做跨单元连字。
- 支持触摸操作。
- 支持按像素流畅滚动，参考 ghostty-android-terminal 实现
- 输入法弹出时终端（包括 helix 等 tui 应用）正确匹配窗口大小不溢出，无字体拉伸/压扁情况

### Shell

- 不检查 Bootstrap 状态，设置下面的环境变量：
  - `HOME` 和 `TERMUX_HOME_DIR_PATH` 为 `/data/data/com.termux/files/home`。
  - `PREFIX` 和 `TERMUX_PREFIX_DIR_PATH` 为 `/data/data/com.termux/files/usr`。
  - `TMPDIR` 和 `TERMUX_TMP_PREFIX_DIR_PATH` 为 `/data/data/com.termux/files/usr/tmp`。
  - `LANG` 为 `en_US.UTF-8`。
  - `COLORTERM` 为 `truecolor`。
  - `TERM` 为 `xterm-256color`。
  - `TERMUX_VERSION` 为 `0.119.0-beta.3`。
  - `PATH` 为 原 `PATH` 附加 `/data/data/com.termux/files/usr/bin`，仅目录存在时附加设置。failsafe 时，保留原 `PATH`，不附加。
  - `ENV` 为 `/data/data/com.termux/.mkshrc`，文件不存在时创建，bash 启动时注入 `PROMPT_COMMAND` 以上报会话标题。
- 不得设置 `LD_LIBRARY_PATH` `PWD` `LD_PRELOAD` 等环境变量
- 仅宿主存在时透传环境变量，不硬编码：`ANDROID_ASSETS`、`ANDROID_DATA`、`ANDROID_ROOT`、`ANDROID_STORAGE`、`EXTERNAL_STORAGE`、`ASEC_MOUNTPOINT`、`LOOP_MOUNTPOINT`、`ANDROID_RUNTIME_ROOT`、`ANDROID_ART_ROOT`、`ANDROID_I18N_ROOT`、`ANDROID_TZDATA_ROOT`、`BOOTCLASSPATH`、`DEX2OATBOOTCLASSPATH`、`SYSTEMSERVERCLASSPATH`。
- 设置中 Shell 启动入口为空时依次寻找 `/data/data/com.termux/files/usr/bin/bash` 和 `/data/data/com.termux/files/usr/bin/login` 以及 `/system/bin/sh`，文件存在即启动，不检查文件权限，失败输出错误，无其他回退。
- shell 崩溃（非主动正常退出）保留现场不关闭会话（参考 Termux，如执行 `exit -1` 后输出 `[Process completed (code 255) - press Enter]`，不主动关闭会话），正常退出时（如 `exit` 命令或用户点击关闭按钮）关闭会话。
- 回滚行数默认 2000，可通过 `termux.properties` 的 `terminal-transcript-rows` 设置（100–50000），非法值回退 2000。
- 较快的启动速度，能够在启动动画结束后直接显示 Shell 和主题背景，而不是从黑屏花费较长时间过渡。
- 没有任何内建命令，不内置提供 `termux-setup-storage` `termux-open` 等，也没有替代实现。
- 忽略 bell character，不做出响应
- 应用位于后台时能够继续活跃，但不实现 `termux-wake-lock`，不得影响“睡眠”等机制。

### 修饰键栏

- 不能和系统全面屏手势冲突：底部上滑不触发按键动画或点击；手势被系统取消时不产生按键。
- CTRL、ALT 等粘滞键点击切换一次性激活，发送带修饰键的按键后消费一次性状态。激活反馈为文字/背景变化与短促缩放，不做双击持久或其他复杂操作。
- 动画/逻辑与 Termux 基本一致，动画简短、不复杂、不卡顿、快速、不浪费时间。
- 固定 2 行 7 列；默认布局在 Termux 上游基础上将 `/` 与 `-` 替换为 DRAWER 与 SCROLL 入口。
- 支持左右滑动在按键页与文本输入页之间切换,和 Termux 实现一致。
- 修饰键栏不被输入法遮挡，在输入法弹出/隐藏时跟随移动。
- **文本搜索输入框**：当文本搜索时，文本搜索输入框取代修饰键栏位置，显示文本输入框、当前位置/总匹配数、上一个、下一个、关闭等按钮。
  - 使用 `libghostty-vt` 的 `search::Search` 实现，命中、导航与高亮均以上游为单一来源。
  - 超出查询长度直接判为无命中且不截断，命中规模上限走上游，不另设命中表。
  - 搜索存在延迟，避免浪费资源，避免未完成输入时搜索
  - 搜索覆盖回滚区而不只是当前屏幕。
  - 匹配到的单元格反色；当前选中命中特殊高亮并居中显示，其余命中保持反色。
  - 计数在未选中时显示 `共 N 个`，选中后显示 `第 k 个，共 N 个`。
  - 上一个、下一个滚动到匹配位置，适应输入法弹出时的显示区域

## 侧边面板（合理布局，不可溢出）

- **会话列表**：列表可能超出可显示范围，可滚动列表，参考 termux-app 实现，保持一致
  - 每一项包括“会话序号、终端标题”（点击切换会话并关闭面板，斜体显示终端标题，不支持修改终端标题），关闭按钮。示例：[1] ~
  - 会话序号从 1 开始递增，序号动态更新。
  - 列表只在打开时刷新，标题由一次批量查询按会话顺序取回。
  - 切换会话时清空上一会话的选区。
- 添加会话按钮。
- “重置终端”按钮，重置终端状态以恢复卡住的终端，清除回滚与滚动状态。终端页面应该干净，没有残余内容。
- 文本搜索按钮。
- 显示 / 隐藏输入法按钮。
- 设置按钮。

## 应用快捷方式 shortcuts

- 只有“新建会话（Failsafe）”一项，图标和 termux-app 一致，应用已启动和未启动时都能使用，使用 `/system/bin/sh` 作为启动入口

## 软件

- 应用启动时检查应用数据兼容性：无法解析或引用不存在的主题/字体时清除该项并回退默认；不得清除 `/data/data/com.termux/files`。
- 实现文档提供器，向系统文件选择器暴露 `/data/data/com.termux/files/home`，可读可写，可执行文件被修改后不丢失权限。
- 参考 termux-app 实现前台通知常驻，显示会话数量，提供“退出”按钮，申请通知权限。
- **不得修改用户文件**：`/data/data/com.termux/files` 由用户管理，应用自身不得修改 `/data/data/com.termux/files/home`，不得创建 `/data/data/com.termux/files/home/.termux/fonts` `/data/data/com.termux/files/home/storage` `/data/data/com.termux/files/home/storage/shared` 等路径，对于 `/data/data/com.termux/files/usr` 只有在 Bootstrap 安装阶段可以作为重命名目标，其他情况不得修改。不自动清理 `/data/data/com.termux/files/usr/tmp` 等目录。
- 适配 Android 软件启动屏动画
- 遵循 Material Design 3
- 声明文件权限（不提示，允许用户自行授权），支持 `/sdcard` 目录
