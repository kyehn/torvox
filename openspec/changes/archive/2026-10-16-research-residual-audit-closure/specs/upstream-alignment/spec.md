## MODIFIED Requirements

### Requirement: 禁改区待维护者裁定项台账（只登记，不实施）

`docs/specification/TESTING.md` 要求而现行门禁缺席的四项，全部落在 AGENTS.md
禁改区（`scripts/`、`.github/`、`android/app/build.gradle.kts`）。维护者已裁定
「都不改，只登记」，落地方案记录于此，不擅自实施：本表各项 MUST 维持登记状态，
未经维护者明确同意 MUST NOT 实施。

登记 MUST NOT 只有落地方案而无事实基线：日后获授权实施的人 MUST NOT 需要重新调研
「改之前是什么样」。故每项 MUST 同时登记**实测基线**与**实施时的验收判据**，
使门禁的判定以基线为准而非以「无输出」为准。

| 项 | 规范依据 | 落地方案（未实施） |
| --- | --- | --- |
| aislop / jscpd 代码检查门禁 | TESTING.md 环境节 | `scripts/check.nu` 与 CI 增 `npx aislop@latest scan` 与 `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步 |
| rapidocr OCR 自动化 | TESTING.md 环境节 + 外部基准优先级 4 | `scripts/` 增 OCR 判定脚本 + `build.yml` 模拟器步骤调用 |
| CI 产物哈希比对 | vercel-labs-wterm 研究结论 | `build.yml` 增产物 sha256 记录步骤 |
| Android 15+ 16KB 页 `.so` 对齐 | GlobalTechInfo-RedTerm 研究结论 | `android/app/build.gradle.kts` 按 AGP 16KB 对齐指南调整打包 |

实测基线（本轮实跑所得，工具版本可能前进，重跑 MUST 以当日实测覆盖本列）：

- **aislop**：`aislop scan --json` 退出码 0，`score` 90、label `Healthy`，
  `summary` 0 errors / 14 warnings / 0 fixable；14 条 `diagnostics` 全部是
  `complexity/*`（11 条 `complexity/file-too-large`、3 条
  `complexity/function-too-long`：`pump_sessions`、`render_inner`、`run_inner`），
  `findingAssessment.byKind` 为 `style-policy` 14 / `confirmed-defect` 0，
  confidence 分布 high 0 / medium 14 / low 0。
  **覆盖面限制**：`coverage.supportedFiles` 42、`unsupportedFiles` 125、
  `dominantUnsupported` 为 `Kotlin`——该工具扫不到本仓 Android 侧 Kotlin 代码，
  故本项 MUST NOT 被当作全仓门禁；它只对 Rust 侧有判别力。
  验收判据：接入后 `confirmed-defect` 与 `errors` MUST 维持 0，
  `file-too-large` / `function-too-long` 的条数 MUST NOT 上升（基线各 11 / 3）；
  该工具归类为 style-policy 的项 MUST NOT 阻断构建，
  只有 error 级或 `confirmed-defect` 才阻断。
- **jscpd**：`jscpd --min-lines 8 --min-tokens 60` 对 `native/src` 与
  `android/app/src/main` 实测退出码 0，31 处克隆 / 423 行重复 / 0.78%，涉 106 个文件。
  验收判据：重复率 MUST NOT 高于该基线；新增克隆 MUST 逐处给出「不修」的具体理由
  （被测行为不同、脚手架样板、或传状态必需的形参列表），MUST NOT 以「先记着」放行。
- **rapidocr**：`flake.nix` 引导调用 `scripts/download-rapidocr-models.nu` 把模型
  下载到 `/tmp/.rapidocr-models`，模型环境已就绪；但
  `scripts/test-emulator.nu` 只跑 `connectedDebugAndroidTest` 与
  `connectedBenchmarkReleaseAndroidTest`，仓内无任何 OCR 消费方，即
  `TESTING.md` 的「拉截图 → 跑 rapidocr → 断言」闭环仍只存在于人工步骤。
  验收判据：实施后 MUST 有程序化断言比对识别文本，MUST NOT 只落截图或只打印识别结果。
- **CI 产物哈希比对** 与 **16KB 页对齐**：本轮未实测（MUST NOT 以推定数字入表）。
  前者实施判据以产物 `sha256` 记录步骤存在且随每次构建产出为准；
  后者须以 AGP 官方指南与实际打包产物校验为准，MUST NOT 以本仓既有配置反推。

#### Scenario: 禁改区项不被静默实施

- **WHEN** 本轮或后续复查接触到上述四项
- **THEN** MUST 维持登记状态并按维护者裁定执行，MUST NOT 以「规范要求」为由擅改
      禁改区文件

#### Scenario: 门禁的覆盖面限制被如实记录

- **WHEN** 引用 aislop 的结果作为本仓代码质量结论
- **THEN** MUST 同时写明它不覆盖本仓 Kotlin 代码（`dominantUnsupported` 为
      `Kotlin`），MUST NOT 据其 0 error 声称全仓无问题

#### Scenario: 接入门禁时以基线而非无输出判合格

- **WHEN** 日后获授权把 aislop 或 jscpd 接入 `scripts/check.nu` 或 CI
- **THEN** 判定 MUST 以上表的实测基线为阈值（重复率不升、`confirmed-defect` 维持 0），
      MUST NOT 以「命令无输出」或「退出码 0」当作已通过
