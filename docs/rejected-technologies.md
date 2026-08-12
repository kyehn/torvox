# Rejected Technologies & Features

> 本文档记录在 `docs/reference/` 调研中被**明确拒绝 / 决定不吸收**的技术与特性，附出处，供后续评审对照，避免反复争论或误入已否决方向。
> 维护规则：新增拒绝项时必须给出 `docs/reference/` 内的出处（文件+行号/章节），并注明拒绝原因。被吸收的旧条目移动到对应吸收文档并在本文档标 `（已吸收）`。

---

## 1. 终端引擎/状态机层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 1 | termux Java 终端模拟器、termux-kotlin 的 Kotlin VT 状态机 | `research-termux-app-extra.md` §6、`research-termux-kotlin.md` §9 | libghostty-vt 已全面超越，维护两份状态机是纯负担 |
| 2 | 手写 VT 解析器（terminator/termx/onecode） | `research-mid-repos-a.md` §5.2 | 重复造轮子，正确性/性能均不如 libghostty-vt |
| 3 | zed 的 gpui 平台层、alacritty_terminal 引擎、Zed settings 体系 | `research-zed-port.md` §9.4 | 平台层与终端引擎均与 torvox 架构冲突 |
| 4 | fission 全家桶（widget 树/IR/布局引擎）、Bevy/Slint、winit/egui/OpenXR | `research-fission.md` §5.2、`research-mid-repos-a.md` §1.5、`research-wgpu-example.md` §8 | 通用 UI 框架引入巨大复杂度和依赖面；torvox 自研渲染管线更贴合终端需求 |
| 5 | Split Panes / Block 模型 | `research-all-projects.md` §P2、`research-warp-extra.md` §11 | Block 模型与 libghostty-vt 冲突，不建议 |

## 2. 图形/渲染层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 6 | ash 直接 Vulkan（wgpu 30 已封装） | `research-warp.md` §7、`research-warp-extra.md` §11 | wgpu 30 已封装实例/适配器/管线，ash 直接操作收益低且易错 |
| 7 | 无限 LOD 网格、程序化几何生成器（raytracing） | `research-wgpu-example.md` §6.1/§6.2 | 终端渲染不需要，需先加深度纹理等基础设施，纯娱乐 |
| 8 | CI sccache | `research-wgpu-example.md` §6.3 | 当前 CI 规模下收益不明显，且引入缓存失效调试成本 |

## 3. 部署/安装层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 9 | bootstrap zip 的 sha256 sidecar 校验 | `research-warp-extra.md` §9.3、`research-warp.md` §5 | 用户明确不需要；bootstrap 通过 HTTP 下载，安装路径已有状态机保证原子性（BootstrapInstaller staging），sha256 校验增加部署复杂度且不解决核心风险 |
| 10 | bootstrap zip 内嵌离线安装 | `research-termux-app-extra.md` §5.7、`research-warp-extra.md` §11 | 用户明确不需要；禁止内嵌，bootstrap 必须支持 HTTP 下载和本地文件安装两种外部来源 |
| 11 | ply 的 `curl \| sh` 无校验安装（反模式） | `research-small-repos.md` §2.5 | 安全反模式；torvox bootstrap 走独立安装器 |
| 12 | 多用户检查 | `research-termux-app-extra.md` §5.7 | 与 torvox 单用户终端定位冲突，Android 已有多用户隔离 |
| 13 | 跨仓库 path 依赖结构 | `research-warp-extra.md` §11 | torvox 单仓库 + generated-patches 更优，跨仓库破坏原子提交 |

## 4. 网络/SSH 层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 14 | SSH + TOFU 全栈（russh/sshj） | `research-mid-repos-b.md` §2.5、§5.3 | 用户明确不需要（当前立项范围外）；TOFU 主机密钥管理复杂且易被绕过 |
| 15 | TermX 的 X11/VNC/SSH/SFTP 服务器、Cron、HTTP 服务器 | `research-mid-repos-a.md` §4.7、§5.2 | 反面教材：重复造轮子，游离于终端核心价值 |
| 16 | proot 用户态方案 | `research-other-repos.md` §3、`research-small-repos.md` §2.3 | torvox native ELF + linker 性能更优 |
| 17 | proot 发行版 | `research-mid-repos-a.md` §2.6、§5.2 | 当前不建议，与 Termux bootstrap 定位冲突；若做则复用 DistroRegistry→init.sh 骨架 |

## 5. 安全/隐私层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 18 | MCP 同意门控弹窗（AgentConsentManager 模型） | `research-haven-extra.md` §2、§19（:333-364, :425） | 用户明确不需要；torvox 的 MCP server 开关即足够，弹窗打断流水线操作 |
| 19 | 隐私黑屏覆盖（切后台黑层防截屏） | `research-small-repos.md` §3.5、§5（:413, :427, :601, :610） | 用户明确不需要；与终端应用"切后台保持可见状态"的体验冲突 |
| 20 | 指纹锁（AppLock） | `research-mid-repos-a.md` §2.6 | 用户明确不需要；与终端快速切换体验冲突 |
| 21 | 指纹/隐私（悬浮窗终端、开机脚本） | `research-mid-repos-a.md` §5.2 | 用户明确不需要 |
| 22 | jni_fn 宏消除手写导出名风险 | `research-wgpu-in-app.md` §6-2（:101, :156, :176, :184, :188） | 用户明确不需要；torvox 手写导出名已有测试覆盖，宏引入第 3 方代码生成依赖 |

## 6. 凭据/会话层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 23 | 会话元数据持久化/重启恢复 | `research-mid-repos-a.md` §3.6 | 用户明确不需要；终端会话重启恢复价值低，状态丢失风险高 |
| 24 | 输出导出到文件 | `research-mid-repos-a.md` §2.6 | 用户明确不需要；终端输出导出可通过重定向自行完成 |
| 25 | 粘贴确认对话框 | `research-gnome-console.md` §4 | 用户明确不需要；打断粘贴流水线 |

## 7. UI/功能层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 26 | 标签条 / 顶部栏、主题编辑器、extra keys 宽度/副键/编辑器 | `research-ghostty-android-extra.md` §5-6/3.7、§7 | 用户明确不需要标签条/顶部栏；主题编辑器与 extra keys 编辑器属大工程，当前不做 |
| 27 | tmux 集成 | `research-mid-repos-b.md` §2.5 | 用户明确不需要；tmux 是外部工具，集成收益低且易碎 |
| 28 | SFTP 断点续传 | `research-mid-repos-b.md` §2.5 | 用户明确不需要；与 SSH 层一同拒绝 |
| 29 | 悬浮窗终端、开机脚本 | `research-mid-repos-a.md` §5.2 | 用户明确不需要 |
| 30 | AI 集成 | `research-warp-extra.md` §9.7、§11 | 用户明确不需要；warp_ai_mobile 为架构基准但当前不实现 |
| 31 | 自定义字体文件导入（TerminalFontStore 4 槽位文件选择） | `research-ghostty-android-extra.md` §5-1、§7 | **文件导入/私有存储复制**（TerminalFontStore.java）拒绝 —— Rust 侧渲染不接受 Android Typeface，文件导入需 native 存储管理，成本高收益低。**同族 bold/italic 面查找 + 像素合成**已吸收（见 §8 #39） |

## 8. 已吸收（历史条目，保留对照）

| # | 条目 | 吸收位置 |
|---|------|---------|
| 32 | wgpu_hal/naga 日志降噪 | `native/src/android/mod.rs` `module_filtered`（已实现） |
| 33 | 行级脏缓存 | `native/src/render/invalidation.rs`（已实现） |
| 34 | 捏合缩放 | `android/.../TerminalSurface.kt`（已实现） |
| 35 | 自定义主题链路 | `android/.../TerminalTheme.kt` / `SettingsRepository.kt`（已实现） |
| 36 | 多击选择（双击选词/三击选行） | `android/.../TerminalSurface.kt`（已实现） |
| 37 | 背景图 复制私有存储 + 失效自愈 → 已实现为背景图设置路径 | `android/.../SettingsScreen.kt`（已实现） |
| 38 | 搜索覆盖层 + 防抖 | `android/.../TextSearchBar.kt`（已实现） |
| 39 | 同族 bold/italic 面查找 + 像素合成 | `native/src/render/font/pipeline.rs` `glyph_information_styled` + `resolve_style_face`（round-231 T6） |
| 40 | TORVOX_BACKEND / TORVOX_POWER 环境变量 GPU 覆盖 | `native/src/render/wgpu_backend.rs` `parse_backend_env` / `parse_power_env`（吸收自 wgpu-in-app） |
| 41 | log_panics hook（panic → logcat） | `native/src/android/logging.rs` `install_panic_hook`（吸收自 wgpu-in-app） |

---

## 变更记录

- 2026-08（本轮）：初版建立，汇总 docs/reference 中全部明确拒绝项；同族 bold/italic 面查找吸收并在 §8 登记；TORVOX_BACKEND/TORVOX_POWER + log_panics 吸收登记。
- 2026-08（修正）：#31 和 #39 措辞修正——"多字体族名设置"改为"同族 bold/italic 面查找 + 像素合成"（FontPipeline 只有单一 fontFamily，通过 resolve_style_face 在同族内查找 bold/italic face）。新增 #40（环境变量 GPU 覆盖）、#41（log_panics hook）。