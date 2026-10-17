# Proposal

## Why

`docs/research` 两份文档集（退役前快照：编号集 31 份 + 现行报告 74 份，合计 105 份）
的**内容**已由 `2026-10-16-research-per-doc-resolution` 逐份处置完毕，但全仓仍有 5 处
**未勾选**的 tasks 项悬着：其中三处是「实际已完成却没勾」，一处是归档顺序与实际不符，
一处是真机环境缺口。这些留白让后来者无法区分「已做未记」与「没做」——而这两种情况
必须用完全不同的方式处理。

同时 `docs/specification/TESTING.md:23-24` 明文要求的 `aislop` / `jscpd` 门禁与
`rapidocr` OCR 自动化，在 `scripts/check.nu` 与三个 CI 工作流中确认缺席。这三项都落在
AGENTS.md 禁改区，维护者已裁定「都不改，只登记」。问题在于：现有登记只有**落地方案**，
没有**实测基线**——日后若获授权实施，仍要重跑一遍工具才能知道「改前是什么样」，
而 `aislop` 对本仓 Kotlin 代码不覆盖这一覆盖面限制至今无人记录，极易被误当作全仓门禁。

## What Changes

- **按事实补齐 5 处未勾选项**（逐处写明依据，不臆断）：
  - `2026-10-15-research-residual-closure` 第六节三轮完整审阅：该变更自身未执行，
    审阅工作由后续 `research-ledger-closure` 第五节与 `research-docs-retirement`
    第三节承接并留下六轮记录，按承接关系补勾并注明承接方，不虚记为本变更所做。
  - `2026-10-15-research-docs-retirement` 归档核对项：两个变更实际在**同一提交**
    归档（提交标题「openspec: 归档 research ledger closure 与 research docs
    retirement 变更」，与该 tasks 写的「先 ledger-closure，本变更后」不符），
    按 git 事实补勾并写明偏差。
  - `2026-10-12-audit-research-residual-gaps` 真机渲染确认：本宿主具备 KVM 但模拟器
    镜像为部分下载且下载包损坏，无法起模拟器，缺口从「环境未探索」升级为
    「阻塞点已探明」，维持未勾选并写明具体错误。
- **`upstream-alignment` 的「禁改区待维护者裁定项台账」补入实测基线**（MODIFIED）：
  - `aislop scan --json` 实测：score 90 / Healthy，0 errors / 14 warnings，
    全部为 `complexity/*` 且评级 `style-policy`、confidence `medium`、
    0 `confirmed-defect`；**覆盖限制**：`supportedFiles=42`、`unsupportedFiles=125`、
    `dominantUnsupported=Kotlin`——该工具扫不到本仓 Android 侧 Kotlin 代码，
    MUST NOT 被当作全仓门禁。
  - `jscpd --min-lines 8 --min-tokens 60` 实测：31 处克隆 / 423 行 / 0.78% /
    106 文件，退出码 0，可作重复率回归基线。
  - `rapidocr` 模型由 `flake.nix` 引导下载到 `/tmp/.rapidocr-models`（环境已就绪），
    但 `scripts/test-emulator.nu` 只跑 `connectedDebugAndroidTest` 与
    `connectedBenchmarkReleaseAndroidTest`，仓内无任何 OCR 消费方。
  - 每项补一条**实施时的验收判据**（以基线为准而非以「无输出」为准），使日后授权
    实施的人不必重新调研基线。
- **禁改区维持登记不动**：本变更 MUST NOT 改 `scripts/`、`.github/`、
  `android/app/build.gradle.kts` 任何文件，只登记实测事实。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `upstream-alignment`：其「禁改区待维护者裁定项台账」Requirement 正文补入
  实测基线、工具覆盖面限制与实施验收判据（登记内容扩充，裁定与禁令不变）。

## Impact

- Affected specs: `openspec/specs/upstream-alignment/spec.md`（MODIFIED 一条 Requirement）。
- Affected docs: `openspec/changes/archive/2026-10-15-research-residual-closure/tasks.md`、
  `openspec/changes/archive/2026-10-15-research-docs-retirement/tasks.md`（补勾与依据）、
  `openspec/changes/archive/2026-10-12-audit-research-residual-gaps/tasks.md`（缺口写明）。
- Affected code: 无。禁改区 MUST NOT 改动。
- 行为变化：无。本变更只补登记与勾选事实，不改任何运行期行为。
