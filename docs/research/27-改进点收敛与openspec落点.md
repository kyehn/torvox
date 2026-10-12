# 27 改进点收敛与 openspec 落点

研究文档全部通读后的可执行收敛：哪些已覆盖、哪些在途、哪些禁止、哪些未声明。§1 与 §5 的结论已被逐条复核修正。

## 0. 范围与方法

- 29 份：01–25 单仓深研、00 汇总、生态深度研究报告、本文与 `28-复审循环与残留问题收敛.md`，
  另 `docs/libghostty.md`（原 26 API 清单）已移出研究目录。
- 方法：三组只读分析 + 对“允许但无覆盖”项的亲自实现核验；引用格式为路径:行号。
- 口径：已覆盖 = 规范或在途变更已有对应条款；在途 = 2026-10-11-complete-ime-editor-contract；允许 = 合规但无新能力（实现指导/方法类）；禁止 = 撞 PROHIBITED；未声明 = DESIGN 无对应功能且 STYLE 禁止擅自实现。

## 1. 结论（已被复核修正）

- 原结论「新增 openspec 能力：0」不成立：其口径把「规范或在途已有条款」直接等同于已覆盖，
  未逐条拿回实现核验。复核后新增能力 1 项（`grid-extraction-incremental`），另有 3 处
  残留（振铃注释失真、手势不采用依据不足、回归口径失实引用）与 2 处声明冲突。
- P1/P2 其余条目仍为实现指导，不新增能力。
- 纠正记录一：应用系统组曾判「先杀前台进程组无覆盖」，经复核 `pty-lifecycle` 25–35 已覆盖挂断-轮询-升级语义，该误判作废。
- 纠正记录二：本节原「全部可落地行为均已覆盖或在途」作废，以 `28-复审循环与残留问题收敛.md`
  的缺失清单为准。

## 2. 已覆盖或在途（无需新立项）

| 改进点 | 来源 | 落点 |
| --- | --- | --- |
| 搜索上限/折叠/导航/居中 | ghostty、charon、anoterm | `text-search` 全篇 + DESIGN 161–166 |
| 设置 live 推全部/spawn 仅新建 | terminator、monet | `settings-propagation` 全篇 |
| 选择交互（取词/拖柄/翻转/边缘滚动/吸附/菜单） | ghostty、charon、ThothTerm | DESIGN 126–136 + `text-selection` |
| PTY 像素 winsize/无变化跳过/挂断收尾/IUTF8 | ghostty、on3、puppy | `pty-lifecycle` 全篇 |
| 退出保留现场文案 | termux、terminator | DESIGN 147 + `shell-entry` 56–67 |
| 按键走上游编码 + 粘滞简化 | ghostty | `upstream-alignment` 按键编码表项 + DESIGN 154–159 + `modifier-bar-sticky-encoding` |
| VT 线程串行 + 轻量 JNI | ghostty | DESIGN 41/43 + `upstream-alignment` 490–507 |
| CJK 回退/取样/locale 补装/孤立代理 | anoterm、rafacodephi | `cjk-rendering` 全篇 + TESTING 31/42 |
| 渲染稳定性全条 | ghostty、anoterm | `render-stability` 全篇 |
| 回滚 2K/备屏不进回滚/样式保留 | termux、anoterm | DESIGN 148 + TESTING 32 + `terminal-state-regression` |
| 链接经 Ghostty 查询 | charon | DESIGN 135–136 |
| 会话列表刷新/节流/序号标题 | ThothTerm、RedTerm | `session-list` 全篇 + DESIGN 170–173 |
| 修饰键栏 2x7/滑入文本框/跟随输入法 | RedTerm、ThothTerm | DESIGN 154–160 |
| Shell 入口框/探测链 | ReTerminal | DESIGN 89–93 + `shell-entry` 全篇 + `settings-propagation` 24–29 |
| 环境白名单/ENV 私有 rc | ReTerminal、RedTerm | DESIGN 100–109 + `shell-env` 全篇 |
| 文档提供器 home 根/树 URI/投影 | ReTerminal | DESIGN 184/186 + `documents-provider` 全篇 |
| Bootstrap 原子换入/staging 清理/postinst | RedTerm、ThothTerm | `bootstrap-install` 全篇 + DESIGN 95–110 |
| 主题固定表/预览/跟随 | ghostty、monet 对照 | `theme-configuration` 全篇 + DESIGN 81–87 |
| 字体三族/列表/字号上下限 | anoterm、ghostty 对照 | `font-selection` 全篇 |
| 通知常驻带退出（标题持久化除外） | RedTerm、ThothTerm | DESIGN 185 |
| IME 填坑/Gboard/ACTION_MULTIPLE | charon、anoterm | 在途 IME 变更（22/28） |
| 宿主链接口径 | 8 仓均无相关内容 | `host-linkage` 全篇 |
| 宽表不自研（论据） | kotlin-app | DESIGN 41 + TESTING 31 |

## 3. 允许但无新能力（实现指导，不立项）

- PTY fork 步骤核对（setsid/TIOCSCTTY/关多余 fd/白名单重装/exec 诊断）：实现在 `pty.rs` 已有，属实现指导。
- UTF-8 粘包跨块保持：`output_processor.rs` 的扫描器已跨调用保持状态，Ghostty 解析本就是状态机；实现指导。
- 读线程合并投递/按帧上限：`session.rs` 的按帧上限已有；实现指导。
- 长按重复时序：`ModifierBar.kt` 已有 400/80ms 的 Termux 对等值；组字删除分流在 IME 在途变更内。
- 纯函数可单测/用例组织/文档清单结构：方法类，按 TESTING 现有体系执行，不新增规范。
- OSC133/预测线/补全/通知音等新语义：未声明，需先补 DESIGN，本次不立项。

## 4. 明确拒绝（禁止/未声明）

| 类别 | 依据 |
| --- | --- |
| 自定义主题/主题编辑器/背景图透明模糊 | PROHIBITED 17/50 |
| 会话持久化/恢复 | PROHIBITED 10 |
| 内嵌 proot/预装发行版/chroot/X11 | PROHIBITED 19–21 |
| Bootstrap 附带哈希校验/自动删备份/收据回滚 | PROHIBITED 8；`bootstrap-install` 39 |
| 自研 VT/Canvas/CPU 回退/沉重 FFI | DESIGN 41/43/50 |
| 双击三击/锚点移动/链接白名单/可用性检查 | DESIGN 132/135–136；PROHIBITED 7 |
| 键栏布局编辑器/功能键页/粘贴确认/local echo | PROHIBITED 11–12/42–48 |
| 光标样式闪烁/回滚可调/启动目录/Shizuku-su | PROHIBITED 30–40/36/38/52 |
| 自定义环境变量/环境配置文件/LD_*/外部命令接口与执行器/MCP/AI | PROHIBITED 13–16/18/52；DESIGN 15/108 |
| 改 home/落 HOME 模板/共享存储启动 | DESIGN 71–74/184/186 |
| 上游 VT 语料金标回放/自验证反向采纳/随仓数据/联网获取 | `upstream-alignment` 92–110/157–209；TESTING 5 |
| 正则搜索/自研扫描/大小写开关 | `upstream-alignment` 418；DESIGN 161–166 |
| JSON 配置导出导入 | `upstream-alignment` 489–507（仅 pollEvent 一处保留） |
| 快捷磁贴/诊断落盘/字体导入/插件小部件 | 无声明（STYLE 53–54） |

## 5. openspec 落点

- 原记「本次不新增变更」作废：复核后新增能力 1 项，落在在途变更
  `2026-10-12-audit-research-residual-gaps` 的 `grid-extraction-incremental`。
- 输入方向仍由在途 IME 变更覆盖；其余为实现指导。
- 后续若立项，须先补 DESIGN 声明，再走 openspec 新变更；不得复活 §4 禁止项。
