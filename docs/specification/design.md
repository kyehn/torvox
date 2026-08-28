# 设计

## 架构

### Rust

- Rust crates

```text
root workspace/
├── Cargo.toml
├── native/
│   ├── Cargo.toml
│   └── src/
│       ├── *.rs
│       ├── android/  ← JNI exports (ffi.rs)
│       ├── render/
│       └── terminal/
│       ├── test/
├── exec-bin/
└── integration-tests/
```

- Ghostty as the Single Source of Terminal State.

- Render entirely in Rust via wgpu. Kotlin receives only lightweight events through direct JNI. No grid data crosses the FFI boundary. 

- One thread per terminal session, produces flat cell arrays. A shared render thread consumes them and drives wgpu.

- Session lives in Rust, not Kotlin. PTY fork/exec, the Ghostty Terminal, and the render loop are all Rust-managed. Android's Activity lifecycle requires the app to handle: **Activity recreation** (screen rotation, config change): Surface destroyed and recreated. The old `ANativeWindow` pointer becomes invalid. **Process death**: The OS can kill the app process. All Rust state and PTY processes die with it. **Background → Foreground**: The app must resume rendering without destroying the terminal state.

- GPU Vulkan rendering (no CPU/OpenGL fallback)

- Upstream `libghostty-vt` / `libghostty-vt-sys` pinned at git master, no local patches

- Clipboard Integration: Read and write the system clipboard from terminal sequences (OSC 52) and user interactions

### Kotlin

- Logs must be visible in Android's `logcat` for debugging while avoiding performance overhead in the render hot path.

- applicationId = "com.termux"

- AOSP testkey (`android/app/aosp-testkey.p12`); self-signing forbidden

## 设置

- 字体大小设置条：字体默认大小设置需要考虑常见设备分辨率（参考 termux），根据设备实际分辨率需要对字体大小设置范围进行限制

- 字体设置：字体未手动设置时默认使用系统等宽字体。对于使用cjk的环境（读取系统设置）检查主字体是否支持cjk，如果不支持需要设置cjk fallback，检查系统默认字体是否支持cjk，如果支持使用其作为cjk fallback字体。字体列表允许从字体文件选择，字体列表不显示如“系统默认”等模糊情况。

- 实际字体信息框：显示字体实际使用情况，包括主字体，cjk fallback 情况，字体大小，单元格

- 光标闪烁开关

- 光标闪烁速度条，范围/精度需要限制

- 光标样式：方块 竖线 下划线

- 软件主题：“日间” “夜间” “跟随系统”三种

- 终端主题：针对终端页面和修饰键栏，默认“Dracula Plus主题”，支持用户自定义主题，“跟随系统”开关 指跟随 “软件主题“ 提供日/夜两种终端主题切换

- 终端启动入口

- 终端回滚行数设置条，范围/精度需要限制

- bootstrap 支持url和本地文件安装，提供termux预设选项。

- “清除应用数据” 按钮

## 终端

- 修饰键栏默认布局跟随termux（基本一致），支持左滑和右滑，左滑显示第二个修饰键栏，右滑显示文本输入框（参考termux）

## 侧边面板

- 会话列表

- 添加会话按钮

- 文本搜索按钮

- 显示/隐藏输入法按钮

- 设置按钮

## 软件

- 应用启动时检查应用数据兼容性如果存在问题可以清除应用数据，确保可以正常启动，设置提供 “清除应用数据” 按钮，应用数据 和 用户数据 不同，除bootstrap设置外不得修改用户数据。

## 测试

- Mesa lavapipe 提供 Vulkan 环境

- 字体设置大小和实际大小测试

## 禁止实现

- 选择菜单 ◀/▶ 锚点移动项
- bootstrap zip 的 sha256 sidecar 校验
- 自定义环境变量: 不通过环境变量接收用户设置或自身传递数据
- 会话数据持久化/恢复
- 粘贴确认对话框 
- 实体键盘快捷键设置
