# 深度研究：GNOME Console 补充 — 亲自逐文件阅读

> 研究日期：2026-08-06 | 项目链接：https://gitlab.gnome.org/GNOME/console
> 前置：`research-gnome-console.md`（kgx-tab.c 搜索 UX，亲自读）；本文补充 **kgx-process.c 268 行全 + kgx-paste-dialog.c 199 行全 + kgx-settings.c 结构**

## 1. kgx-process.c（268 行，完整阅读）

**进程信息抽象**（libgtop 封装）：
- `KgxProcess { pid, parent, euid, argv }`（:56-61）
- `kgx_process_new(pid)`（:99-110）：`kgx_pids_get_pid_info` 取 parent/euid
- `kgx_process_get_is_root`（:126-132）：**euid == 0 判断 root**
- `kgx_process_get_argv`（:139-153）：**惰性加载 cmdline**（首次访问才读 /proc）
- `kgx_process_get_title`（:156-181）：**argv 拼接为标题**，超长（MAX_TITLE_LENGTH=100）截断并返回 "Process %d" 标题 + 截断 argv 作副标题（`kgx_str_constrained_append`）
- `kgx_process_get_list`（:211-243）：**GTree（GPid → KgxProcess）**——注释说明 0.1.0 用 GPtrArray 改为 GTree 更快查找

**用途**（模块注释 :25-31）：`KgxApplication` 监控终端内运行的进程 → **根据进程是 root 来调整窗口样式**（标题栏红色等）。

**torvox 对比**：torvox 无进程监控/标题栏样式。**"shell 内运行 root 进程 → UI 提示"是安全 UX 候选**（P2）。torvox 的 MCP terminal_info 有 shell pid，可扩展 euid/argv。

## 2. kgx-paste-dialog.c（199 行，完整阅读）

**粘贴确认对话框**（AdwAlertDialog）：
- 触发场景（标题 :88）："You are pasting a command that runs as an administrator"——**仅在粘贴内容以 root 相关命令运行时显示**（调用方判断）
- **内容截断到 8000 字符**（:62 `kgx_str_constrained_dup(self->content, 8000)`）显示在正文："Make sure you know what the command does:\n%s"
- 按钮：Cancel / **Paste（destructive 样式）**（:94-98）
- 异步 API（:112-133）：`kgx_paste_dialog_run` + `run_finish`——GTask 模式

**torvox 对比**：torvox 无粘贴确认。**安全粘贴（检测 sudo/su 开头的多行粘贴 → 确认对话框）是安全 UX 候选**（P2）。termux 也没有——这是 GNOME 系特色。

## 3. kgx-settings.c（829 行，结构扫描）

- 属性枚举（:69）+ GObject 属性系统
- 字体设置回调（:484-524）：`font_desc` 解析
- `kgx_settings_set_custom_shell`（:649）：自定义 shell
- 大量 GSettings 键绑定回调

**torvox 对比**：torvox 用 DataStore + SettingsScreen（Compose），架构不同。GNOME 的 GSettings 模式（系统级配置）不适用 Android。

## 4. 结论

gnome-console 补充阅读确认两个 P2 候选：**root 进程 UI 提示**（kgx-process.c）和**管理员命令粘贴确认**（kgx-paste-dialog.c）。两者都是安全 UX 增强，非核心功能。

## deep-v1 增量（2026-08-07：kgx-terminal + kgx-palette 精读确认）

### 规模更正
gnome-console src/ 共 **13439 行 C**（GTK4 + VTE），核心文件：kgx-terminal.c（1030）、kgx-window.c（730）、kgx-palette.c（430）、kgx-livery.c（425）、kgx-font-picker.c（318）。

### kgx-terminal.c（1030 行）——VTE 包装
- `vte_terminal_set_colors`（:151）主题应用
- **URI 属性提取**（:189-278）：`vte_terminal_ref_termprop_uri`（file/directory/either）+ `vte_terminal_check_hyperlink_at`（OSC 8 超链接）+ `vte_terminal_check_match_at`（正则匹配）——点击检测三通道（URI/超链接/正则）
- **torvox 对照**：torvox 链接点击在 Kotlin 侧（UrlDetector 正则）——VTE 的 OSC 8 超链接元数据 torvox 无（ghostty 引擎有 hyperlink 支持，未接 Kotlin）——P3 记录

### kgx-palette.c（430 行）——调色板序列化
- GVariant 三字段（foreground/background/colours）+ deserialise/serialise_to 往返 + 缺失字段 g_warning 降级
- **torvox 对照**：torvox theme JSON（54 字节 setTheme 打包）同理念——确认

### 新增汇总
| # | 发现 | 级别 |
|---|------|------|
| 1 | OSC 8 超链接元数据（VTE termprop）torvox 未接 Kotlin | P3 |
| 2 | palette GVariant 往返 vs torvox 54 字节 setTheme | 等价 |
