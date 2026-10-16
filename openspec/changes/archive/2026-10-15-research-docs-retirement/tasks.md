# 任务

## 一、索引表（规范增量）

- [x] `upstream-alignment` 新增 Requirement「研究报告逐份索引与文档可移除性」
- [x] 索引表录入 74 行：73 份逐仓报告 + `ecosystem.md`，另加旧编号文档集处置行
- [x] 逐行核对：五组子代理并行审计 73 份报告，逐条比对报告内容与
      索引行/台账；发现并修掉两类缺口——依据指向空表（分屏、Web 壳语料、libtsm 语料
      等）与结论仅存于待删报告或归档（konsole/pebrel/KetraTerm/oryxis/hollow/mintty
      等语料行，WebSocket/ZMODEM/命令面板/远程会话/AI 集成/vi-like/文件管理器/
      结构化输出/第三方渲染实现/Terminal API 等能力行）
- [x] 索引行依据全部可在本规范内解析：`语料判定补齐行` 33 行、`能力裁定补齐行`
      37 行、`参照面与反面做法收敛补齐行` 15 行、`禁改区待维护者裁定项台账` 4 行、
      既有台账与能力 spec 按名引用
- [x] 脚本校验：`docs/research/*.md` 74 份逐份在索引表中命中（`comm` 双向差集为空）

## 二、文档退役

- [x] `ecosystem.md` 的参照面评级、禁止项、测试边界三节内容并入索引表与既有台账：
      评级语义与「生态参照面组行」定义写入 Requirement 正文，禁止项三条与测试边界
      由 Requirement 正文与 `terminal-state-regression` 承接，两处悬问闭合结果
      （shitty 许可双轨、ratty font/atlas）与字体表真值对照位置一并登记
- [x] 删除 `docs/research/` 全部 74 份（`git rm -r docs/research`，目录随之移除）
- [x] `FontFileOverrideInstrumentedTest` 注释改指 `openspec/specs/upstream-alignment` 的
      禁改区待维护者裁定项台账（rapidocr OCR 自动化未实施），不再引用 `docs/research`

## 三、在途变更收尾

- [x] `2026-10-15-research-ledger-closure` 第三节后的禁改区四项：确认已登记，勾选
- [x] 三轮完整审阅第一轮（审计：逐份事实核对 5 处出处错 + 2 处存疑、对抗式 9 份
      丢结论、`ecosystem.md` 12 条缺口，均已在索引行与台账内修复）
- [x] 第二轮对抗式审阅：出处归属 3 处更正（`curl | sh` 归 Ply、整包校验为 BLAKE3 与
      sha256 双实现、`ReleaseStringUTFChars` 为 AW/AnsolePlus/kotlin-app 三仓）；terminator
      三项出处改归 on3-terminal 两例共识与 `AndroidThin` 复查台账；`ecosystem.md` 逐句落点
      审计（107 行全文）后发现并补齐 8 处无落点断言：AnsolePlus 孤立高代理项越界未修、
      玩具组空 `catch` 与 su/Shizuku 提权、puppy 构建产物入库、on3 宽表缺 `isLowSurrogate`、
      kosh/ReTerminal 文档提供器形态、charon「全文本框镜像」不采纳、kotlin-app 测试套件
      非正例、ghostel FFI 细节
- [x] 第三轮完整审阅（终态，17 行索引抽查 + `ecosystem.md` 107 行逐句复核 + 上游事实
      4 项核验）：补修 5 处——主规范台账 `shafinthedev/ReTerminal` owner 更正为
      `RohitKushvaha01/ReTerminal`、ledger-closure proposal 份数口径改 105（31 + 74）、
      Ply 行号收窄为 `src/main.rs:102`、单列 fuzz 语料路径 `test/fuzz-libghostty`、
      「性能门禁行」锚点名对齐为「设备性能自动化门禁行」
- [x] 第四轮审阅（能力裁定补齐行 37 行逐行审计 + owner 抽查 41 个）：补修 3 处——
      设备性能门禁判定与现状不符（宏基准已存在并接入 CI，缺的是阈值判定）、
      `STYLE.md` 已声明界面仅简体中文（非「未声明」）、主规范 `jackpal/androidterm`
      → `jackpal/Android-Terminal-Emulator`（旧名 API 404）
- [x] 第五轮审阅（归档可行性仿真 + 自洽性）：补修 10 处——两份 tasks 的勾选状态（ledger
      第二／三次审阅）与验证记录（含 `-- -D warnings` 写法、`androidTest` 源集口径）、
      子代理分组计数笔误、`context.rs` 历史行号改现况、`level-poll` 自造术语改
      `new_output`（`AtomicBool`）、ledger proposal 的 Requirement 名与 `ecosystem.md`
      退役后表述、retirement proposal 的「主 spec 已有」与「在途新增表」归属、
      两份 spec delta 内 `docs/research` 的「已退役」口径
- [x] 第六轮审阅（本仓事实断言核验 10 项）：全部成立（行号、常量、仓库名、CI 接线），
      未发现需改的规范缺陷
- [x] `openspec validate --all`（33 通过 / 0 失败）与 markdownlint（`openspec/**/*.md`
      277 文件 0 issues）通过；`--strict` 的 9 项告警全为「>500 字符长 Requirement」，
      属既有风格
- [ ] 归档 `2026-10-15-research-ledger-closure` 与本变更（顺序：先 ledger-closure，
      本变更索引行的依据表才可解析），核对主 spec 条目数

## 四、验证

- [x] `cargo clippy --all-targets -- -D warnings`、`cargo test`（全目标）通过：
      lib 621 + concurrency 2 + external_benchmarks 9 全绿，0 失败
- [x] `./gradlew testDebugUnitTest` 与 `./gradlew compileDebugAndroidTestKotlin` 通过
      （被改注释文件 `FontFileOverrideInstrumentedTest.kt` 属 `androidTest` 源集，
      单测任务不编译它，故另编译该源集）
- [x] 删除后全仓 grep 确认无 `docs/research` 悬空引用（归档变更与本变更自身描述除外；
      `native/target/` 构建产物内的无关命中不计）
