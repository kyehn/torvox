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
- [ ] 删除 `docs/research/` 全部 74 份
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
- [ ] 第三轮完整审阅确认无遗留
- [ ] `openspec validate --all`、markdownlint 通过
- [ ] 归档 `2026-10-15-research-ledger-closure` 与本变更（顺序：先 ledger-closure，
      本变更索引行的依据表才可解析），核对主 spec 条目数

## 四、验证

- [ ] `cargo test`（全目标）与 `cargo clippy --all-targets --deny warnings`
- [ ] `./gradlew testDebugUnitTest`（Kotlin 单测，含被改注释文件所在模块）
- [ ] 删除后全仓 grep 确认无 `docs/research` 悬空引用（归档变更除外）
