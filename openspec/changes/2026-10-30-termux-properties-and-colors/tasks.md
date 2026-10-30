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

## 遗留

- [ ] `PROHIBITED.md` 中「禁止解析 termux.properties/colors.properties」与现状冲突，
      属受保护文件，待用户决定是否删除该条
- [ ] `soft-keyboard-toggle-behaviour = enable-disable` / `hide-soft-keyboard-on-startup`
      / `disable-terminal-session-change-toast` 三项不实现，理由见 proposal
