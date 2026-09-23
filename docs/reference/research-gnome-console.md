# 深度研究：GNOME Console (kgx)

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/gnome-console`（depth 1）
> 语言：C（GTK4 + VTE）；GPL-3.0
> 定位：GNOME 官方轻量终端。核心价值在 **VTE 集成的搜索 UX 细节**（narrowing-down 处理）——VTE 是 libghostty-vt 同级的成熟终端库

## 1. 项目结构

```
src/
├── kgx-terminal.c (1030)   # VTE 终端封装（复制/粘贴/超链接/选择回调）
├── kgx-tab.c (1232)        # 标签页 + 搜索栏（核心研究目标）
├── kgx-pages.c (949)       # 页面管理
├── kgx-window.c (730)      # 主窗口
├── kgx-settings.c (829)    # 设置
└── kgx-despatcher.c (435)  # 命令分发
```

## 2. 搜索 UX：`kgx-tab.c:191-250`（最高价值参考）

VTE 不自动高亮搜索匹配，也不允许手动选择文本，kgx 的"创造性"方案：

```c
search_changed (GtkSearchBar *bar, KgxTab *self) {
    search = gtk_editable_get_text (...);
    // 全小写 == 原文 → 大小写不敏感
    if (!g_strcmp0 (lowercase, search)) flags |= PCRE2_CASELESS;
    regex = vte_regex_new_for_search (g_regex_escape_string (search, -1), -1, flags, &error);
    // narrowing-down 判断：last_search 包含当前 search（收窄中）
    narrowing_down = search && priv->last_search &&
                     g_strrstr (priv->last_search, search);
    g_set_str (&priv->last_search, search);
    if (!narrowing_down)
        vte_terminal_search_find_previous (...);  // 扩展：先找上一个
    vte_terminal_search_set_regex (..., regex, 0);
    if (narrowing_down)
        vte_terminal_search_find_previous (...);  // 收窄：设 regex 后找上一个（保持当前匹配）
    vte_terminal_search_find_next (...);
}
```

**关键洞察**（注释原文）：buffer 是 "foo bar baz"，当前高亮 "baz"，按退格变 "ba" → "bar" 中的 "ba" 也匹配。如果先 find_previous 再设 regex，会跳到 "bar" 而不是留在 "baz"。所以：
- **收窄搜索**（`g_strrstr(last_search, search)` 为真）：先设 regex 再 find_previous → 留在当前匹配
- **扩展搜索**：先 find_previous 再设 regex → 从头找

**对比 torvox**：torvox 的 `searchAllInScrollback`（Rust 侧）返回全部匹配，Kotlin 侧 `TextSearchFindMatches` 处理。torvox 的搜索框（SearchBarView）在输入变化时重新搜索——是否处理了"收窄时保持当前匹配"？**这是 UX 缺口**。

## 3. 选择/复制：`kgx-terminal.c`

- `copy_activated`（:486-489）：`vte_terminal_get_text_selected()` 取选区文本
- `select_all`（:542）：`vte_terminal_select_all()`
- **动态启用**（:705-708）：
  ```c
  static void kgx_terminal_selection_changed (VteTerminal *self) {
      gtk_widget_action_set_enabled (GTK_WIDGET (self), "term.copy",
                                     vte_terminal_get_has_selection (self));
  }
  ```
  无选区时 Copy 菜单项禁用（`selection_changed` 信号驱动）。**对比 torvox**：torvox 的 ActionMode 菜单 Copy 总是可用——无选区时点击无效果。可借鉴动态禁用。
- 超链接：`vte_terminal_check_hyperlink_at(x, y)`（:278）、`vte_terminal_ref_termprop_uri`（OSC 8 属性 URI）、`copy-link` action（:910）——右键菜单有"复制链接"
- 鼠标自动隐藏：`vte_terminal_set_mouse_autohide(TRUE)`（:942）

## 4. 其他参考点

- `kgx-paste-dialog.c`：粘贴确认对话框（安全粘贴提示，类似 bracketed paste 确认）——kgx 在粘贴时显示对话框（多行内容确认）
- `vte_terminal_search_set_wrap_around(TRUE)`（:943）：搜索环绕

## 5. 依赖

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| VTE（libvte） | 终端核心 | 不适用（libghostty-vt 更现代） |
| GTK4 + libadwaita | UI | 不适用（Android/Compose） |
| PCRE2 | 搜索正则 | 参考（torvox 用 fuzzy/case-insensitive 简化搜索） |

## 6. 代码注释引用（待加入 torvox 代码）

```
TextSearch 或 SearchBarView (torvox):
// 搜索收窄保持当前匹配参考 gnome-console kgx-tab.c:191-250
// narrowing_down = g_strrstr(last_search, search)；收窄时设 regex 后再 find_previous
TerminalSurface/菜单:
// Copy 动态启用参考 gnome-console kgx-terminal.c:705-708
// selection_changed 时 action_set_enabled(copy, has_selection)
```

## 7. 结论

gnome-console 提供两个 UX 细节：**搜索收窄时保持当前匹配**（VTE 无 API 时的创造性方案，可移植到 torvox 的 TextSearch）和 **Copy 动态禁用**。粘贴确认对话框是安全 UX 参考。
