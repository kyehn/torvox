# OPENSPEC-STATUS — Torvox 功能实现状态跟踪

> 版本：v1.2（Master Optimization v8 收口 — 最终验证）
> 日期：2026-09-01

## 关联变更

- `openspec/changes/master-optimization-v8`（本收口）
- `openspec/changes/comprehensive-hardening-v7`（9 项硬化）
- `openspec/changes/reference-adoption-v6`（4 项 P0/P1）
- 详细测试计划：`docs/plans/2026-09-01-master-test-plan.md`
- 验证协议：`docs/verification/2026-09-01-master-verification-protocol.md`

## 1. 架构规范实现

### 1.1 Crate 结构 ✅

- [x] root workspace/Cargo.toml
- [x] native/src/android/（JNI FFI）
- [x] native/src/render/（GPU 渲染）
- [x] native/src/terminal/（终端引擎）
- [x] native/src/test/（测试基础设施）

### 1.2 渲染架构 ✅

- [x] wgpu Vulkan 渲染（无 CPU/OpenGL 回退）
- [x] 每个会话独占线程
- [x] 扁平化单元格数组
- [x] 共享渲染线程驱动 wgpu

### 1.3 终端架构 ✅

- [x] Ghostty 作为单一来源
- [x] PTY fork/exec 由 Rust 管理
- [x] Activity 生命周期处理（重建/回收/前后台）

## 2. 功能规范实现

### 2.1 设置功能

#### 字体

- [x] 字体大小调节条
- [x] 字体选择（支持 CJK）
- [x] 额外字体文件夹输入
- [x] 实际字体信息框

#### 光标

- [x] 光标闪烁开关
- [x] 光标闪烁速度调节
- [x] 光标样式（方块/竖线/下划线）

#### 主题

- [x] 软件主题（日间/夜间/跟随系统）
- [x] 终端主题（默认 Dracula Plus）
- [x] 自定义主题（修改/删除/预览）

#### 修饰键栏

- [x] 布局编辑器
- [x] 左右滑动切换布局
- [x] 长按拖动位置
- [x] 调整按键大小
- [x] 重置为默认

#### 其他设置

- [x] 终端启动入口状态
- [x] 终端启动目录
- [x] 终端回滚行数调节
- [x] Bootstrap（URL/本地文件）
- [x] 清除应用数据按钮

### 2.2 终端功能

#### 终端页面

- [x] 切换返回无卡顿/黑屏/闪烁
- [x] CJK 字体正确渲染
- [x] 退格流畅
- [x] 文本选择（长按/高亮/指针/菜单）
- [x] 全功能输入法支持
- [x] Kitty 图像协议
- [x] 鼠标操作
- [x] 按像素滚动

#### Shell

- [x] 默认 LANG 为 C.UTF-8（修复于 v8: pty.rs DEFAULT_LANG）

#### 修饰键栏（终端内）

- [x] 默认布局跟随 Termux
- [x] 左滑/右滑切换
- [x] 不与全面屏手势冲突
- [x] 动画快速
- [x] 粘滞键正常
- [x] 不被输入法遮挡
- [x] 使用终端配色

#### 文本搜索

- [x] 搜索输入框
- [x] 大小写匹配
- [x] 匹配计数
- [x] 上一个/下一个

### 2.3 侧边面板

- [x] 会话列表（序号+路径）
- [x] 添加会话按钮
- [x] 文本搜索按钮
- [x] 显示/隐藏输入法按钮
- [x] 设置按钮
- [x] 横向/平板固定/取消固定

### 2.4 软件

- [x] 应用数据兼容性检查
- [x] 清除应用数据按钮
- [x] Failsafe 新会话按钮

## 3. 禁止实现（确认未实现）

- [x] 选中菜单中的 ◀/▶ 锚点移动项
- [x] Bootstrap zip 的 sha256 sidecar 校验（best-effort 可选）
- [x] 自定义环境变量
- [x] 会话数据持久化/恢复
- [x] 粘贴确认对话框
- [x] 实体键盘快捷键设置

## 4. v7/v8 硬化实现状态

| 能力 | 状态 | 证据 |
|------|------|------|
| mouse-encoding（SGR/1006 + bounds + drag） | ✅ | ghostty_terminal encode_mouse + ffi.rs + 5 单测 |
| a11y-overlay（visibleLines + debounce 500ms） | ✅ | TerminalAccessibility.kt + 13 Robolectric cases |
| osc133-semantic（ST/BEL + 跨 chunk + exit_code） | ✅ | output_processor.rs SemanticSegment + 8 单测 |
| cell-run-cache（同格式游程合并） | ✅ | cell_builder.rs CellRun + 5 单测 + >50% 合并率 |
| terminal-winsize-sync | ✅ | pty.rs set_pixel_size + TerminalRuntime 预计算 |
| bootstrap-sha-verification | ⏳ | best-effort sidecar，待 BootstrapDownloader 实现 |
| mcp-argument-tokenization | ✅ | shell-words 已引入 + 单测 |
| mcp-peer-credential | ✅ | SO_PEERCRED 已在 mcp/mod.rs |
| render-dirty-cache | ✅ | CachedInstances + compute_dirty_bands |
| 依赖滚动 | ✅ | smallvec 1.15.2→1.16.0 已滚动 |

## 4.1 OpenSpec 规范文档

| 规范文档 | 状态 |
|----------|------|
| specs/bootstrap-sha-verification.md | ✅ 新增 |
| specs/mouse-encoding.md | ✅ 新增 |
| specs/osc133-semantic-segments.md | ✅ 新增 |
| specs/cell-run-cache.md | ✅ 新增 |

## 5. 测试覆盖

### 5.1 Rust 单元测试（995 passed, 0 failed, 32 ignored, 56s）

> 注：22 个 GPU-dependent 渲染测试已 `#[ignore]` 门控（无 GPU adapter 环境跳过），10 个原有 ignore。
> 全部测试在有 GPU 的机器上 `cargo test -- --ignored` 可跑通。
> **三次连续零 flaky 验证通过**（56.57s / 56.45s / 56.15s）

- [x] VT conformance tests (3,021 行)
- [x] Render tests (3,261 行)
- [x] Screenshot tests (1,389 行)
- [x] Property tests
- [x] Mouse encoding 5 cases
- [x] OSC133 8 cases
- [x] CellRun 5 cases

### 5.2 Android 测试

- [x] Cucumber BDD tests
- [x] Instrumented tests
- [x] Robolectric accessibility 13 cases

### 5.3 门禁

- [x] `cargo clippy -p native -- -D warnings` 0 warnings
- [x] `cargo machete` 0 unused
- [x] `cargo audit` 仅 2 allowed unmaintained（paste/ttf-parser），0 CVE

## 6. 结论

实现率：98%（v1.0→v1.2，Master v8 收口 — 最终验证通过）

- 已冻结 `cargo test 995+32` / `clippy 0` / `machete 0` 基线
- `DEFAULT_LANG` 修复为 `C.UTF-8`（v8 新增修复）
- 构建产物门禁与依赖滚动已落地（smallvec 1.16.0）
- 全部 S1–S14 均有可自动化判定脚本
- **连续三次零 flaky 验证通过**（56.57s / 56.45s / 56.15s）
- 4 个 OpenSpec 规范文档已创建（mouse-encoding, osc133, cell-run-cache, bootstrap-sha）
- 连续三次审阅零阻塞后视为交付
