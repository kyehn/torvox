# 深度研究：warp-mobile-android (ImL1s)

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/warp`（depth 1）
> 语言：Rust（crates/android-host）+ Kotlin；AGPL 许可
> 定位：Warp 终端移动版（含 AI agent 功能），核心价值在 **PTY 的 async-signal-safe 实现** 与 **bootstrap 原子安装**

## 1. 项目结构

```
warp/
├── crates/android-host/     # Rust JNI 宿主（本项目重点）
│   ├── lib.rs (2394)        # JNI 导出（AI agent 流式、PTY、bootstrap、IME）
│   ├── vulkan.rs (2263)     # ash 0.38 直接 Vulkan swapchain 生命周期
│   ├── pty.rs (413)         # fork/exec PTY（AS-safe 黄金参考）
│   ├── bootstrap.rs (788)   # bootstrap zip 原子安装
│   ├── font_render.rs (701) # cosmic-text 字体发现/栅格化 harness
│   ├── ime.rs (159)         # InputConnection → JNI commitText
│   └── terminal_model.rs (302)
├── android/app/             # Kotlin UI（Compose）
└── spikes/                  # vulkan-surface-recreate、symlink-jnilibs 验证 spike
```

## 2. PTY：`crates/android-host/src/pty.rs`（黄金参考）

### 2.1 fork 前的准备工作

`pty.rs:46-104`：
1. `openpty()` 获取 master/slave
2. 用 RAII guard 包装 fd（`FdGuard`，`:20-30`）
3. `FD_CLOEXEC` on master（防 exec 泄漏）
4. **fork 前预构建所有 CStrings**（`prog_cstr`、argv、envp）——注释明确 "pre-build all CStrings BEFORE fork"

### 2.2 child 路径（fork 后只允许 AS-safe 调用）

`pty.rs:106-160`：
```rust
0 => {
    // ── child: only AS-safe calls after this point ───
    unsafe {
        libc::setsid();
        libc::ioctl(slave_fd, libc::TIOCSCTTY.into(), 0i32);
        // V1-prep defensive hardening: seed a non-zero TTY window size
        // so any TIOCGWINSZ before MainActivity sends its first
        // PTY_RESIZE returns a usable 24×80 instead of 0×0. zsh's ZLE
        // module logs a warning and falls back to a degraded
        // line-editor on 0×0 (per zsh source: Src/Zle/zle_main.c).
        let ws = libc::winsize { ws_row: 24, ws_col: 80, ... };
        libc::ioctl(slave_fd, libc::TIOCSWINSZ.into(), &ws);
        libc::dup2(slave_fd, 0); libc::dup2(slave_fd, 1); libc::dup2(slave_fd, 2);
        if slave_fd > 2 { libc::close(slave_fd); }
        libc::close(master_fd);
        libc::execve(prog_cstr.as_ptr(), argv_ptrs.as_ptr(), envp_ptrs.as_ptr());
        // execve 失败：AS-safe 写 errno 到 stderr（已 dup 到 slave → master → logcat）
        #[cfg(target_os = "android")]
        {
            let errno = *libc::__errno();  // Bionic 的 AS-safe errno 访问器
            let msg = b"warp-pty: execve failed errno=";
            libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
            // 手写数字转换（不分配）
            ...
            libc::_exit(127);
        }
    }
}
```

**对比 torvox（pty.rs）**：
| 方面 | warp | torvox |
|------|------|--------|
| 子进程 errno 报告 | AS-safe `write(2)` 数字手写转换 | `_exit(100 + errno)` 由 wait thread 解码（`session.rs`） |
| 窗口尺寸 seed | 24×80 `TIOCSWINSZ` | 由调用方 resize |
| fork 前 CString | 预构建 | 同（本项目同样预构建） |
| execve 失败码 | `_exit(127)` | `_exit(100+errno)`（信息更多） |
| SELinux 方案 | 直接 execve（需自己的 exec 机制） | linker64 + LD_PRELOAD（termux-exec 风格） |

warp 的 `write(2)` errno 数字转换比 torvox 的 exit-code 编码更直接（logcat 立即可见），但 torvox 的 100+errno 方案在 wait thread 中也能解码。**两者都是正确的 AS-safe 模式**。

## 3. Bootstrap：`crates/android-host/src/bootstrap.rs`（原子安装参考）

`bootstrap.rs:1-48` 设计文档：
1. 读 `version.json` sidecar（含期望 sha256）从 APK assets
2. 若 `$PREFIX/.bootstrap-version.json` 已存在且 sha256 匹配 → 早退（已安装）
3. 读 `assets/warp/bootstrap/bootstrap-aarch64.zip`
4. 解压到 `usr.tmp/`（**原子性**：kill-mid-extract 后 usr.tmp 不完整，但不会"看起来已安装"）
5. 应用 `SYMLINKS.txt` sidecar（termux 格式：`target←linkname`）
6. 写 `.bootstrap-version.json` 作为安装标记
7. 返回码：0=成功/已安装、3=sha256 不匹配、4=IO 错误

**对比 torvox（BootstrapInstaller.kt）**：torvox 同样用 staging 原子安装（stagingDir 注释），但缺少 version.json/sha256 校验（有 `detectDpkgVersion` 之类）。warp 的**安装标记 = sha256 校验文件**方案更健壮（可检测 zip 损坏）。**建议 torvox 采用 sha256 sidecar 校验**。

## 4. Vulkan：`crates/android-host/src/vulkan.rs`（ash 直接 API）

与 torvox 的 wgpu 不同，warp 用 **ash 0.38 原始 Vulkan**。核心经验（对本项目 wgpu 用户仍有价值）：
- swapchain recreate 触发：`VK_ERROR_OUT_OF_DATE_KHR` / `SUBOPTIMAL_KHR`（`acquire_next_image` 返回 `Ok((idx, suboptimal))`——suboptimal 在 ash 0.38 不折叠进 Err，:726-729）
- Present mode FIFO（vsync-locked）；image count = min_image_count + 1
- per-image present-wait semaphores（避免 image 重用竞争，:86-94）
- `VK_LAYER_KHRONOS_validation` debug 构建默认开
- spike 结论：Adreno 750 p95=18ms、Adreno 660 p95=28ms（100 次 swapchain recreate）
- `unsafe impl Send for VulkanSurface`（:109-111）：ANativeWindow 引用计数（NDK 合约）

**对比 torvox**：torvox 用 wgpu（抽象掉 swapchain 细节），wgpu 内部处理 OUT_OF_DATE。warp 的直接 ash 方案代码量大（2263 行）但完全可控——本项目不需要（wgpu 30 已封装）。

## 5. 字体：`font_render.rs`

- **NDK API 29+ `ASystemFontIterator`** 发现系统字体，fallback 到 `/system/fonts/` 扫描
- 载入 `cosmic_text::fontdb::Database`（`Source::File` + collection index）
- `cosmic_text::SwashCache::get_image_uncached` 栅格化
- 字体 fallback 分类 `classify_text_runs`（ASCII/CJK 分段）

**对比 torvox**：torvox 用 `fontdb` + swash（同 cosmic-text 家族），字体发现走 `set_extra_font_paths` + 系统扫描（`font_db.rs:28-63`）。warp 的 `ASystemFontIterator`（NDK 原生）比 Java `Typeface` 枚举更直接。**torvox 可考虑 ASystemFontIterator 替换 /system/fonts 扫描**（减少 JNI 往返，minSdk 29+）。

## 6. IME：`ime.rs`

- `commit_text(text, new_cursor_position)`、`set_composing_text`、`finish_composing_text` JNI 导出
- Java 侧 `WarpInputView ── BaseInputConnection override ──► JNI`

**对比 torvox**：torvox 的 IME 走 Compose `BaseInputConnection` + `InputCoalescer`（Kotlin 侧），Rust 侧只有 `feedPty`/`writeKey`。架构不同（torvox 的输入处理在 Kotlin 层更灵活），但 warp 的 composing 状态显式管理（`set_composing_text`/`finish_composing_text` 分开）值得注意——torvox 的 InputCoalescer 是否有 composing 泄漏问题需自查。

## 7. 依赖清单

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| ash 0.38 | 原始 Vulkan | 不适用（wgpu 已抽象） |
| cosmic-text / fontdb / swash | 字体 | **已用**（同栈） |
| zip | bootstrap 解压 | 可用（torvox 用 java.util.zip.ZipInputStream，Rust 侧解压可省 JNI 往返，但收益小） |
| tokio-util | CancellationToken | 不适用（torvox 无 tokio 运行时在主路径） |
| jni crate | JNI | **已用**（同栈） |

## 8. 项目文档吸收价值

- `CLAUDE.md` / `PROJECT.md` / `TEST_INFRA.md`：warp 有极详细的 agent 协作文档体系（项目状态、测试基础设施、验收标准）——**torvox 的 `docs/project-health.md` 可以借鉴其 TEST_READY.md 模式**（明确列出每项能力的验证状态）
- spike 目录（vulkan-surface-recreate）：单点验证实验的模式值得 torvox 借鉴（快速验证风险点）

## 9. 代码注释引用（待加入 torvox 代码）

```
pty.rs child 路径:
// PTY child AS-safe 参考 warp crates/android-host/src/pty.rs:106-160
// 子进程 errno 报告用 write(2) + 手写数字转换（warp 方案）或 exit code 编码（torvox 方案）
BootstrapInstaller.kt:
// bootstrap 安装标记参考 warp crates/android-host/src/bootstrap.rs:1-48
// version.json + sha256 sidecar：检测 zip 损坏；torvox 目前只写版本文件
font_db.rs:
// 系统字体发现参考 warp font_render.rs（ASystemFontIterator, NDK 29+）
// 可替代 /system/fonts 目录扫描，减少 JNI 往返
```

## 10. 结论

warp 是 **PTY AS-safe 实现** 与 **bootstrap 原子安装** 的高质量参考。其 PTY 的 `write(2)` errno 报告与 torvox 的 exit-code 编码是两种正确模式；bootstrap 的 sha256 sidecar 校验值得 torvox 采纳；`ASystemFontIterator` 是字体发现的现代化路径。Vulkan 部分因 wgpu 抽象对本项目参考价值有限。
