# 任务

顺序固定：先补 `upstream-alignment` 的禁改区实测基线，再逐处按事实补勾归档 tasks，
最后闭合真机项。**严格串行**，每小步一次提交。

## 一、规范增量

- [x] `upstream-alignment` MODIFIED「禁改区待维护者裁定项台账（只登记，不实施）」：
      正文保留原禁令与落地方案表，新增「实测基线」小节（aislop 90/Healthy/0 error/
      14 style-policy/0 confirmed-defect + Kotlin 不覆盖；jscpd 31 克隆/423 行/0.78%；
      rapidocr 模型就绪但无消费方；两项未实测显式标注），Scenario 由原有 1 条
      增至 3 条（新增「门禁的覆盖面限制被如实记录」与
      「接入门禁时以基线而非无输出判合格」，原有「禁改区项不被静默实施」原样保留）。
      验证：`openspec validate research-residual-audit-closure --strict` is valid，
      `markdownlint-cli2` 0 issues。

## 二、按事实补齐归档 tasks 的未勾选项

- [x] `2026-10-15-research-residual-closure` 第六节三轮审阅：勾选并写明
      **本变更未执行，承接方为** `research-ledger-closure` 第五节（第一至三次）
      **与** `research-docs-retirement` 第三节（第四至六轮），逐轮给出承接节号。
      验证：grep 该 tasks 第六节读到承接关系与节号，且不出现「本变更已完成」的表述。
- [x] `2026-10-15-research-docs-retirement` 归档核对项：勾选并更正顺序表述——
      两变更实际在同一提交归档（提交标题「openspec: 归档 research ledger closure
      与 research docs retirement 变更」，`git log --diff-filter=A` 核实），
      与原文「先 ledger-closure，本变更后」不符；补记主 spec 条目数实测值
      （`upstream-alignment` 32 条 Requirement / 68 条 Scenario）。
      验证：重跑 `git log --diff-filter=A` 复核提交标题；重算主 spec 条目数与文中数字一致。
- [x] `2026-10-15-research-residual-closure` 第四节两项禁改区待办：勾选为
      **已登记未实施**，并指向本变更写入 `upstream-alignment` 的实测基线。
      验证：grep 该节读到指向常驻规范的基线条目名，非悬空描述。

## 三、真机渲染确认（5.4）

- [x] 探测宿主设备能力：KVM `/dev/kvm` 可用、`system-images/android-35` 存在但仅
      146 字节 `.installData`（无 `package.xml`，avdmanager 不认）、`emulator` 目录
      有 `lib64/` 但缺可执行二进制。补记为「环境已探明、阻塞点具体」而非「环境不可用」。
- [ ] 仪器测试实跑（`:app:connectedDebugAndroidTest` + benchmark）：**阻塞并已探明**。
      `sdkmanager "emulator" "system-images;android-35;google_apis;x86_64"` 解压镜像时
      失败于 `Error on ZipFile unknown archive`（下载包损坏），故无法起模拟器。
      已把探测结果与失败原因写入 `audit-research-residual-gaps` 的 5.4，
      该项从「环境未探索」升级为「阻塞点具体」并维持未勾选。
      **此项不得以主机数字替代**：主机侧 60 帧复用行数一致只能证明复用判据稳定，
      不能替代设备侧的无卡顿/撕裂确认。

## 四、收尾

- [x] 门禁复跑：`scripts/check.nu` 全量退出码 0（含 `cargo fmt/clippy/machete/test
      (623+2+9)/doc/bench/release 构建`、`semgrep` 0 findings、`markdownlint`
      295 文件 0 issues、Gradle `detekt spotlessCheck lintDebug lintVitalRelease
      assembleDebugAndroidTest testDebugUnitTest benchmark baselineprofile`
      BUILD SUCCESSFUL），另 `openspec validate --all` 32 通过 / 0 失败（该数为 31 份
      常驻 spec 加当时仍在本变更目录的这一项；归档后复跑同一命令为 31 项，归档变更
      不计入）、本变更 `--strict` is valid。

      **环境坑（务必按此复现）**：`check.nu` 的 shebang 是
      `#!/usr/bin/env -S nix develop --command nu`，而 `android/build.gradle.kts:19-26`
      要求 `IN_NIX_SHELL=impure`、`NIX_BUILD_TOP` 以 `/tmp/nix-shell.` 开头、
      `nativeBuildInputs` 是长于 1000 的 nix 存储路径。直接跑脚本可满足前两项，
      但若宿主把 `TMPDIR` 指向别处（如本会话的 `/tmp/reasonix-session-tmp-*`），
      `NIX_BUILD_TOP` 会随之改名而落在该前缀之外，Gradle 即报
      `Must run inside nix develop`。复现完整门禁 MUST 用 `TMPDIR=/tmp ./scripts/check.nu`。
      这是宿主环境限制，非仓库缺陷，故未改 `build.gradle.kts`（属「不得轻易更改」清单）。
- [x] 连续三轮审阅（可复现机械复核 + 逐句对码 + 门禁），直至无问题。

      审阅共三轮，第 1 轮与第 2 轮各发现 1 处并已修，第 3 轮发现 4 处并已修，
      第四、五、六次复核均无问题：

      - 轮 1（机械复核）：发现 1 处——tasks 第一节写「与两条 Scenario」有歧义
        （读作总数为两条），改为写明 Scenario 由原有 1 条增至 3 条并逐条列名。
        其余项通过：禁改区 `scripts/`、`.github/`、`android/app/build.gradle.kts`
        与 `docs/specification/` 全程 `git diff` 为空（本轮只动 openspec 目录下
        的文件与一个 Kotlin 测试文件）；delta 完整覆盖原 Requirement 13 行正文、
        0 行遗漏、原有 Scenario
        原样保留；基线数字与实测逐项一致（score 90、14 warnings、11+3 规则分布、
        coverage 42/125/Kotlin）；5.4 的五个断言各有实测出处。
      - 轮 2（对抗式，6 项）：除轮 1 那处外，另 5 项通过——jscpd 数字重跑完全一致
        （31 克隆 / 423 行 / 0.78% / 106 文件，证明可复现而非一次性观测）；
        原 spec Scenario 无遗漏；基线已声明「工具版本可能前进，重跑 MUST 以当日实测
        覆盖」；未实测的两项仍显式标注、未被写成已实测；两条新 Scenario 与正文
        约束互补而非重复。
      - 轮 3（追查更早归档变更的未勾项，发现 4 处并全修）：
        1. `record-libghostty-audit-verdicts` 3.3 `validate --specs` 全绿：实测
           31 passed / 0 failed，补勾。
        2. 同变更 4.1 / 4.2 两项待办：已由 `adopt-upstream-key-encoder` 与
           `adopt-bracketed-paste` 实现并归档。**4.2 的实际做法与初稿相反**——规约
           明写 MUST NOT 取用 `Terminal::paste`（含换行文本返回 `Error::Rejected`），
           故按「目标达成但路径与初稿相反」结案，不得读成照原计划实现。
        3. `complete-ime-editor-contract` 7.3 条目数核对：实测 `ime-editor-contract`
           存在且含 9 条 Requirement / 22 条 Scenario，补勾。
        4. 同变更 4.1–4.3「组字光标锚点未实现」：**结论已被后续实现推翻**——
           §0.3 只扫了 `InputConnection` 上 `@hide` 的同名方法，漏了公开 API
           `InputMethodManager.updateCursorAnchorInfo`（同一 binder 调用）。实现提交
           为 2026-10-14 的「实现组字光标锚点回报」。据此作废 §0.3 结论并按已实现
           补勾；其中 4.3 查明**原先无覆盖**（`ComposingCursorAnchorTest` 仅三条），
           已补 `marker follows the cell metrics instead of caching the old ones` 并
           经变异验证（把 `markerY + cellHeight` 改成硬编码 `20f` 模拟缓存旧度量即判红）。
        门禁：4.3 补测后 `scripts/check.nu` 全量退出码 0（Gradle BUILD SUCCESSFUL、
        semgrep 0 findings、markdownlint 0 issues、cargo test 623+2+9）。

      残留未勾项经复核均属应当保留的类别，无一为「已做未记」：
      本变更的仪器测试（阻塞于模拟器镜像包损坏，已写明阻塞点）、本变更的归档步骤、
      `audit-research-residual-gaps` 5.4（真机项，已升级为「阻塞点已探明」）、
      `complete-ime-editor-contract` 4.5 与 7.2（前者原文即声明「所称即时性单测全仓
      无踪，故不勾选」，后者原文即载明仪器全量 5 红为环境）、以及
      `terminal-improvements` 28 项（整份驳回归档，原文已声明「勾选状态保持未勾以
      表示『未实现』」，逐项留有违禁或已被取代的依据）。
- [x] 归档本变更（归档后以 `openspec validate --all` 复核）。
      归档前 `openspec validate research-residual-audit-closure --strict` is valid、
      `scripts/check.nu` 退出码 0。归档后 delta 并入常驻
      `openspec/specs/upstream-alignment/spec.md`，该 Requirement 的 Scenario 由 1 条
      增至 3 条、条目总数 32→32（MODIFIED 不新增 Requirement）。
