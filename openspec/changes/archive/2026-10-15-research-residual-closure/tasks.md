# 任务

## 一、调研与核对（已完成）

- [x] 通读 `docs/research/` 编号文档集 31 份（用户指定 commit）+ 当前工作区 63 份
- [x] 对照 `openspec/specs` 31 个能力、68 个归档变更与代码现状逐条判定全部可行动项
- [x] 核查 libghostty-vt 钉住的 rev 与上游 master 差距（实测 0 提交落后，钉住即 tip）
- [x] 核对 `TESTING.md` 工具链与覆盖范围逐项落地情况（rapidocr 环境就绪但识别无
      自动化；aislop/jscpd 门禁缺席且 `scripts/` 属禁改区；覆盖项 9 项有测试、
      4 项部分、1 项缺设备测试）
- [x] 抽查编号文档集 10 份 S/A 级单仓深研，验证结论已被规范完全吸收
- [x] 确认 4 处残留缺口并立项本变更

## 二、release 构建档恢复

- [x] `native/Cargo.toml` 恢复 `[profile.release]`：`lto = true`、
      `codegen-units = 1`、`opt-level = "z"`、`strip = true`
- [x] 不恢复 `panic = "abort"`（`jni_export_guard` 依赖 unwind）；不设 dev/bench 档
- [x] `cargo build --release` 验证链接与产物；`.so` 体积与构建时长记录于提交说明

## 三、规范增量

- [x] `upstream-alignment` 新增「OSC 覆盖逐项核对」Requirement 与核对表
- [x] `session-list` 新增「切换活动会话清空全部会话态」Requirement
- [x] `docs/persistence/ecosystem.md`：attachSession 对照项改「已核对」；
      参照面表补入五份外部研究报告结论
- [x] `openspec validate` 与 markdownlint 通过
- [x] 归档本 change 并核对目标 spec 文件条目数（防同级标题内容被静默丢弃）

## 四、禁改区待维护者裁定（AGENTS.md：`scripts/`、`.github/` 不得擅改）

维护者已裁定「都不改，只登记」，故本节两项**已登记且维持未实施**；实测基线与实施
验收判据见常驻规范 `upstream-alignment` 的「禁改区待维护者裁定项台账」Requirement
（补记于 `research-residual-audit-closure`，2026-10-16）。

- [x] **aislop / jscpd 门禁**：`TESTING.md` 第 24 行明文要求，
      `scripts/check.nu` 与 CI 均无该步骤（归档任务自述「规范要求的这道门禁
      实际缺席」）。落地方案：`scripts/check.nu` 增 `npx aislop@latest scan` 与
      `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步，与既有
      semgrep / markdownlint 同段。须用户明确同意后执行。
      实测基线：aislop score 90 / Healthy，0 errors、14 warnings 全为 `complexity/*`
      且 0 `confirmed-defect`，但 `dominantUnsupported` 为 `Kotlin`（不覆盖本仓
      Kotlin）；jscpd 31 处克隆 / 423 行 / 0.78%。
- [x] **rapidocr OCR 自动化**：`TESTING.md` 第 23 行与研究文档优先级 4 要求
      「拉截图 → 跑 rapidocr → 断言」闭环，现仅有人工宿主步骤。落地方案：
      `scripts/` 增 OCR 判定脚本 + `build.yml` 模拟器步骤调用。须用户明确同意后执行。
      实测基线：`flake.nix` 已引导把模型下载到 `/tmp/.rapidocr-models`（环境就绪），
      但 `scripts/test-emulator.nu` 只跑两项 connected 测试，仓内无任何 OCR 消费方。

## 五、文档处置台账（94 份）

口径：已实现 = 结论已有 spec 条款与实现；已禁止 = 撞 `PROHIBITED`/`DESIGN` 明禁；
已登记 = 已有 spec/持久库逐项登记不采纳及理由；不立项 = `DESIGN` 未声明
（`STYLE` 禁止实现未声明功能），理由已记录；本轮解决 = 本变更落地。

份数与范围更正：标题原写「93 份」而两小节为 31 + 63，自相矛盾，实测应为 **94 份**。
两个数字各自可核：编号文档集 31 份；工作区 63 份是本变更**编写时**的快照
（`docs/research` 当时确为 63 份），其后随用户补档增长——本节归档提交时为 72 份，
`docs/research` 退役前为 74 份，故本节不覆盖工作区的全部报告。
31 + 74 = 105 份的最终逐份判定与逐行补齐见
`2026-10-16-research-per-doc-resolution`，其后续审阅与收口为
`2026-10-16-research-residual-audit-closure`；同日更早归档的
`2026-10-15-research-ledger-closure` 承接的是禁改区四项与台账行，
不在 105 份逐份判定链上。本节只记录编写时的判定，不追认增量。

### 编号文档集（用户指定 commit，31 份）

- [x] `00-汇总与可参考点收敛`：P0 全部已实现（bootstrap-install 已归档）；
      P1 为实现指导；禁止项清单与 `PROHIBITED` 一致；openspec 落点已闭环
      （新增能力 `grid-extraction-incremental` 已归档并实现）。
      「shell 引用单引号转义」一条复核为不采纳：入口切分以 ASCII 空白为准
      （`pty.rs` 注释明示无引号语义），与 `DESIGN` 示例写法一致，引号词法属
      未声明功能。
- [x] `01-ghostty-android-terminal`（S）：搜索/选择/PTY 收尾/像素 winsize/
      备份原子暂存均已实现；OSC 8 二次确认经 `text-selection` 明拒（点击即跳转）。
- [x] `02-termux-app`（S）：保留现场文案/键矩阵/staging→rename/回滚 2K/通知
      均已实现；VT 单测向量转写经 `upstream-alignment`「上游 VT 语义不由本仓
      测试覆盖」拒绝。
- [x] `03`–`07`、`11`、`12`、`15`–`17`、`19`、`20`、`22`–`25` 单仓深研：
      可落地结论均已收敛进 `openspec/specs`（见各 spec 的参照），反面结论
      进入禁止清单；后续提交已删除该批文档（信息并入 `docs/persistence/`）。
- [x] `04-terminator`：设置 live/spawn-time 二分、真 PTY、退出保留现场已实现；
      OSC checklist 转为本变更的 OSC 逐项核对表；seccomp fork 变通为
      实现指导（`pty.rs` 已有 fork 步骤核对）。
- [x] `08-Rin`：手写 VT 反面论据已吸收；ENV/读写线程分离已实现。
- [x] `09-ThothTerm`：长按选择/虚拟键/会话刷新/双指缩放已实现；BiDi 已禁止。
- [x] `10-ownerroc-monet`、`13-puppy`、`21-on3`：PTY 细节共识已实现；
      proot/rootfs 已禁止；OSC 133 未声明不立项。
- [x] `14-charon`：IME 五项填坑（ime-editor-contract 已落地组字锚点）、
      选择模型已实现；golden 回放经「可被实现覆写」门槛拒绝。
- [x] `18-anoterm`：CJK 宽度原则、粘包重解、搜索口径均已实现；SSH/tmux 禁止外。
- [x] `27-改进点收敛`：结论已复核修正并全部落地（新增能力 1 项已归档）；
      禁止项表与 `PROHIBITED` 一致。
- [x] `28-复审循环`：二十六轮残留项逐条核对当前状态——组字光标锚点已实现
      （公开 `InputMethodManager.updateCursorAnchorInfo`）；rev 哈希口径已裁定
      （hash 只在 `Cargo.toml` 钉里）；JNI 批量合并已执行（标题/单元格宽高/
      主题目录/会话计数收敛为批量查询）；getMode 导出已删；渲染暂停账本已收敛。
      仍未做项维持原判：`focus_event` 写出半边（结构性不可稳定测，覆盖缺口
      据实登记）、37 项过度复杂（仅记录不立项）。
- [x] `29-减码杠杆`：P0 L1 批量合并已执行；P0 的 517 未用项经复核仅
      `color_scheme` 真未接线（无 DESIGN 消费者，`upstream-alignment` 已登记
      不采用），`size_report`/`modes`/`screen` 均已接线；L2–L6 为有界重构
      建议（P1/P2），不扩大为本次变更；sugaloaf spike 维持不做。
- [x] `30-VT测试边界`：优先级 1–3 已落地（fuzz 语料回放 + UCD 全量宽度 +
      kitty 键盘协议接线）；优先级 4 的 hmtx 真度量已由字体表基准取代，金图
      OCR 留宿主人工（禁改区待裁定，见第四节）；优先级 5 esctest/vttest 维持
      「不扩张成穷举 VT 一致」结论。
- [x] `终端模拟器生态深度研究报告`：§16 P0 八项逐条核对（搜索/下发/选择/
      PTY/IME 均已实现；录制回放已由 fuzz 语料回放覆盖；WcWidth 孤立代理
      由宽度委托上游 + UCD 基准覆盖；Kotlin Termux 对照无行动）；§10 attachSession
      清单本次补 spec 条款；§14 OSC checklist 本次补核对表。

### 当前工作区研究报告（63 份，按可行动项判定归类）

- [x] `Kuddev-pebrel`：profile 优化建议本轮落地；GPT 自研 VT 已禁止；
      vi mode / copy mode 未声明不立项。
- [x] `contour-terminal-contour`：conformance 外部真相候选经「不测依赖行为」
      拒绝（upstream-alignment 已登记）；glyph atlas/文本簇已实现；
      daemon 持久会话已禁止；reflow/synchronized output 未声明不立项。
- [x] `dakra-ghostel`：同 VT 引擎参照；Android PTY 经验已被 pty-lifecycle 覆盖。
- [x] `orhun-ratty`：wgpu present shader 仅参照（本仓渲染已越过该层）。
- [x] `mintty-mintty`：Unicode 宽度表委托上游 + UCD 基准；minibidi/Sixel
      未声明不立项；搜索/剪贴板已实现。
- [x] `migueldeicaza-SwiftTerm`：Ghostty fuzz 语料对照价值已由
      external_benchmarks 落地；测试矩阵为参考。
- [x] `TermoraDev-termora`：控制序列/选择/查找/键编码均已实现或禁止；
      测试对照须外部真相，未采纳。
- [x] `JetBrains-jediterm`：vttest 语料经 terminal-state-regression 禁止项拒绝；
      type-ahead 未声明不立项（InputBatchBuffer 已保序）。
- [x] `borisfaure-terminology`：shell 回归 driver 属外部语料不采纳；
      链接自识别已禁止；主题格式已禁止。
- [x] `felixse-FluentTerminal`：Shell Profile/设置拆分已被等价实现覆盖；
      iTerm 主题导入禁止；TerminalBridge 无 Android 落点。
- [x] `vercel-labs-wterm`：DOM 脏行思路与脏跟踪同源已实现；WebSocket 背压
      无网络终端不适用；可访问性朗读已禁止。
- [x] 其余 52 份（KDE/lxqt/elementary/fyne/domTerm/bcon/odytty/glass/Bobcat/
      libtsm/kmscon/sfte/TerminalFX/zerotty/mars/copad/zt/orzma/con-terminal/
      par-term/par-term-emu-core/go-ansiterm/Tek4010/terminator/RedTerm/kosh/
      ReTerminal/kotlin-app/HiSH/lumina/havoc/aclock/terminal7/qwertty-term/
      iced_term/BossTerm/zashterminal/godot-xterm/ttyphoon/TragicWarrior/
      zebra/fbpad/cutefishos/wilsonglasser/RtlTerminal/omnipty/qterminal/
      yakuake/konsole/terminology 已计、KDE-maui-station 未提交件由所属会话处置）：
      逐份核对可行动项——自研 VT 引入全部禁止；桌面/Web 形态均无 Android 落点；
      命令面板/文件管理器面板/输入广播/linkifier/OSC7 cwd 衍生等未声明不立项；
      TalkBack 已禁止；qwertty 差分语料经采纳门槛拒绝（第三方 VT 差分，
      非上游资产）。结论已沉淀入 `docs/persistence/ecosystem.md` 参照面表。

## 六、审阅循环

本变更未自行执行这三轮审阅；审阅由后续变更承接执行，结论与修补记录见对应节次。
补记于 `research-residual-audit-closure`（2026-10-16），依据为下列承接方记录。

- [x] 第一次完整审阅（对照 docs/research 逐份抽查 10 份 + 全量构建测试）
      ——承接方：`research-ledger-closure` 第五节「第一次完整审阅」（对照两份文档集逐份
      抽查 + 全量构建测试）
- [x] 第二次完整审阅（对抗式：构造可破坏新 spec 条款的输入）
      ——承接方：`research-ledger-closure` 第五节「第二次完整审阅（对抗式）」，
      修补记录转记于 `research-docs-retirement` 第三节
- [x] 第三次完整审阅确认无遗留
      ——承接方：`research-ledger-closure` 第五节「第三次完整审阅确认无遗留」
