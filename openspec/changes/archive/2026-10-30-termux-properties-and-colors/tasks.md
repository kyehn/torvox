# 任务

## 配置层

- [x] `TermuxProperties`（22 键 + termux 默认值）与 `TermuxPropertiesParser`
- [x] `ColorsProperties` 解析与 `toPaletteOverride()`（19 槽，`-1` = 不覆盖）
- [x] `TermuxPropertiesRepository`：`~/.termux/` → `~/.config/termux/` 兜底，失败保留上次结果
- [x] `PropertiesFile`：注释、续行、转义（`\n` `\t` `\r` `\f` `\uXXXX`）
- [x] `TerminalKeyCatalog`：键名→键码、特殊动作、五套字形

## 原生侧

- [x] `Palette::with_overrides`：按槽位覆盖，空槽沿用主题
- [x] `setPaletteOverride`（JNI）：基底主题单独存 `ACTIVE_THEME`，覆盖压在基底之上
- [x] `setCursorStyle`（JNI）→ `set_default_cursor_style`
- [x] `setCursorBlinkRate`（JNI）：只写原子量，不触发 GPU 初始化
- [x] 光标闪烁相位按时基计算，空闲路径以「相位翻转」作为重绘理由
- [x] `on_bell` → `bell_pending` → `Event::Bell`（入队前按会话合并）
- [x] `pwd()` / `getPwd` / `getSessionPwds`

## 界面

- [x] 键栏改由 `extra-keys` 驱动，`extra-keys = []` 时不画也不留高度
- [x] `terminal-toolbar-height` 缩放单键高度，预留高度同源计算
- [x] `terminal-margin-*` 内缩 SurfaceView
- [x] `fullscreen` 隐藏状态栏，`use-fullscreen-workaround` 选择唤出方式
- [x] `night-mode` 覆盖日夜判定（视图与原生同源）
- [x] `bell-character` → 震动/提示音/忽略，两次间隔下限 120ms
- [x] `default-working-directory` 解析（`~`、相对、绝对；尾斜杠归一）
- [x] 会话抽屉在标题为空时显示 OSC 7 目录
- [x] `back-key` / `volume-keys` / `ctrl-space-workaround` / 会话快捷键
- [x] `soft-keyboard-toggle-behaviour`：`enable-disable` 时 KEYBOARD 键翻转软键盘可用性，
      禁用后轻点终端不再拉起输入法（含平台按 `onCheckIsTextEditor` 自动弹的那条路径）
- [x] `hide-soft-keyboard-on-startup`：默认 `true` 不弹；`false` 在首帧后请求一次输入法
- [x] `disable-terminal-session-change-toast`：会话切换顶部提示 `[序号] 标题`，可关闭
- [x] `extra-keys-style`：五档取值与字形集合对齐上游（`default`/`arrows-only`/
      `arrows-all`/`all`/`none`）

## 系统

- [x] Wi-Fi 锁（`WIFI_MODE_FULL_LOW_LATENCY`），与唤醒锁同生命周期
- [x] `VIBRATE` 与 `ACCESS_WIFI_STATE` 权限

## 测试

- [x] properties 解析 18 项、colors 映射 12 项、布局转换 12 项
- [x] 工作目录格式化 7 项、`default-working-directory` 解析 6 项
- [x] 响铃 4 项、闪烁相位 + 空闲重绘门控 2 项
- [x] JNI 冒烟：`getPwd`/`getSessionPwds`/`setCursorStyle`/`setPaletteOverride` 的往返与入参校验

## 规范

- [x] 新增 `openspec/specs/termux-configuration/spec.md`
- [x] 更新 `modifier-bar-height-reservation`（预留高度改为数据驱动）
- [x] 更新 `cursor-rendering`（默认样式与闪烁）
- [x] 把 `termux-config` 的读取契约与三个受控键的校验并入 `termux-configuration`，
      删掉「只允许三个键」的旧要求

## 复查修正

- [x] 工作目录必须落在真实存在的目录上，否则回退家目录（原先直接交给 PTY，会话起不来）
- [x] 数值键越界回退默认而不是钳到边界（上游 `getDefaultIfNotInRange` 的口径）
- [x] 闪烁间隔只接受 0 或 100..2000，越界按「不闪」处理
- [x] 配置文件只认常规文件、不跟随符号链接、超过 64 KiB 跳过
- [x] Wi-Fi 锁改用 `WIFI_MODE_FULL_HIGH_PERF`：低延迟模式在灭屏时不生效
- [x] 光标样式重新接受 `beam` 并落成竖线

## 遗留

- [ ] `PROHIBITED.md` 中「禁止解析 termux.properties/colors.properties」与现状冲突，
      属受保护文件，待用户决定是否删除该条（已记入 `docs/termux-config-review.md`）
