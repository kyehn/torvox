# Swipterm（iOS 终端参考）

## 来源与口径

- 上游：<https://github.com/swipt-automation/Swipterm>，iOS 终端模拟器；2026-11-26 复核 main。
- 依赖取自 `Swipterm.xcodeproj/project.pbxproj`：`Lakr233/libghostty-spm`（`GhosttyKit`/`GhosttyTerminal`/`GhosttyTheme`/`ShellCraftKit`）、`swipt-automation/SwiptermKit`、`groue/GRDB.swift`，均以 Swift Package 引入。
- 结论按 App 源码与两个包的源码复核（浅克隆只读）。仓库还含虚拟机、发行版安装与发布工具链；本文只记录能同本仓终端实现对照的部分，不记临时版本号与提交哈希。
- 使用 Swift 6.2、SwiftUI + UIKit；`libghostty-spm` 提供 C API 的 Swift 封装与 iOS 视图层，`SwiptermKit` 提供 ARM64 客体 Linux 运行时。

## 组成

| 层 | 来源 | 职责 |
| --- | --- | --- |
| 宿主 UI | `Swipterm/Terminal/`、`Settings/`、`Setup/` | 会话管理、搜索栏、主题/字体与快捷键设置、客体发行版安装向导 |
| Ghostty 绑定 | `GhosttyKit`、`GhosttyTerminal`、`GhosttyTheme`、`ShellCraftKit` | 终端状态机与渲染、UIKit 输入/选区/滚动、配置与主题序列化 |
| 客体 Linux | `SwiptermKit` 的 `LibSwipterm`、`MoltenVK`、`KosmicKrisp` XCFramework | 虚拟机、内核与根文件系统启动、virtio 设备、GPU 后端 |
| 偏好与工具 | GRDB、Go `internal/cmd` | `Swipterm.sqlite` 偏好存储、截图与发布命令行 |

## 会话与 PTY 桥接

- `TerminalSession` 每个会话持有一个 `TerminalSurfaceView.SurfaceView`（`UITerminalView` 子类），布局脱离时仍保留，避免丢失网格与回滚；`TerminalViewState.configuration` 设为 `TerminalSurfaceOptions(backend: .inMemory(session))`。
- `InMemoryTerminalSession` 的 `write`/`resize` 闭包接到 `TerminalRelay`：输入经 `AsyncStream<Data>` 写入客体 PTY，视口变化同步到 `LinuxTerminal`；输出循环读 `terminal.read()` 再 `session.receive(data)`。
- 背压：`setOutputBacklogHandler(highWater: 4 MiB, lowWater: 1 MiB)`，输出任务在 `waitWhileBacklogged()` 暂停；进程退出时 `releaseBacklogForExit()` 放行等待者，避免收尾挂死。
- 启动顺序：客体仍在 boot 时首会话订阅启动控制台并喂给 `session.receive`，boot 结束后发清屏序列；随后 `openSession(columns:rows:)`。首次输出后等 0.3s 静默再注入 shortcut 的 `startupInput`（`cd` 加命令，逐行补 `CR`）。
- 生命周期：`Phase` 为 `starting`/`running`/`finished`；正常退出（码 0）延迟 1s 由 `TerminalSessionManager` 关闭会话，列表空时自动补一个。`close()` 同时隐藏 surface、取消焦点、终止 relay。
- `TerminalExtras` 实现标题、cwd、聚焦、关闭、bell、桌面通知、命令结束、颜色变化、打开 URL、剪贴板确认与搜索等 delegate，统一转发到 `TerminalViewState` 或宿主回调。剪贴板确认未接回调时默认拒绝，打开 URL 走独立确认 UI。
- 主题：`TerminalTheme(light:dark)` 取自 `GhosttyThemeCatalog`，或把自定义 `key=value` 逐行喂给 `builder.withCustom`；`AppSettings.terminalAppearanceDidChange` 通知触发 `controller.setTheme` / `setTerminalConfiguration` 热更新，字体族与字号也经 `TerminalConfiguration` 下发。

## 搜索与启动模板

- 搜索不自行解析回滚：`TerminalSearch` 用 Ghostty binding action `search:<needle>`、`navigate_search:next|previous`、`end_search`、`search_selection` 驱动，结果由 `TerminalSearchEvent`（started/total/selected/ended）回灌 SwiftUI。
- `FindQueryPacer` 按查询长度分档：短词延迟 0.5s，中长词立即；输入去掉换行与控制字符并截到 512 字符；`FindHistory` 只留不少于 3 字符的最近 10 条并合并连续前缀；无结果提示延迟 0.4s，选中命中最多等 5s、每 0.3s 重试。
- `TerminalShortcut` 只是启动文本模板：`directory` 与 `command` 拼成 `cd '<path>'` 加命令、逐行补 `CR`；候选从客体 `/etc/passwd`、`~/.ssh/config` 和常见 bin 目录里的 coding agent 推导。

## 输入与触控

- `GhosttyTerminal` 自带 UIKit 侧 `UITextInput`、输入附件栏（sticky 修饰键）、硬件键盘路由、IME 标记文本、触控选区/拖拽/放大镜、平滑滚动与捏合缩放字体；`SurfaceView` 主要覆写宿主行为：空白区点击、输入栏显隐判定、粘贴菜单、拖放文件，以及退出第一响应者前同步焦点。
- 快捷键与修饰键栏可编辑（`ManageShortcutsView`、`TerminalShortcut`）；这是本仓禁止的能力，这里只记录其存在。

## 客体 Linux 与持久化（不移植）

- `LinuxSystem`、`ContainerProvisioner`、`KernelBootstrap` 负责选发行版、下载校验内核包与根文件系统、在客体里执行 `reconcile_kernel`（屏蔽 dracut/grub 安装钩子）；无可引导内核时提示需要 4K 或 16K 页的 ARM64 内核。`GPUMode` 在 automatic、MoltenVK、KosmicKrisp、disabled 间选择。
- `AppDatabase` 用 GRDB 打开 `Swipterm.sqlite`，只有 `preferences(name, data blob)` 一张表；`SQLITE_CORRUPT`/`SQLITE_NOTADB` 时删除相关文件重建，`eraseDatabaseOnSchemaChange = true`；`AppSettings` 存主题、字体、CPU/内存上限、GPU 模式与快捷键。会话内容与回滚不落库。

## 与本仓对照

- 可借鉴：一会话一 surface 的稳定性策略、输出高低水位背压与退出放行、delegate 分类、搜索节流与结果回传、主题热更新。
- 不借鉴：客体虚拟机与 GPU 后端、SQLite 偏好与快捷键编辑、剪贴板确认弹窗；本仓 PTY、渲染与输入都在 Kotlin/Rust 侧完成，`docs/specification/PROHIBITED.md` 禁止会话持久化与自定义快捷键编辑。
- 实现差异：本仓直接用 `libghostty-vt` 的 Rust API（`Terminal::vt_write`、`search::{Search, MatchBuffer}` 等）驱动渲染与 UI，没有 Swift binding action 与 UIKit 视图这一层。
