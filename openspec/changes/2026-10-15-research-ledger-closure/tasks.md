# 任务

## 一、调研与核对（已完成）

- [x] 重读用户指定 commit `9aea751d` 的 `docs/research` 编号文档集 31 份
- [x] 重读当前工作区 `docs/research` 73 份
- [x] 对照归档变更 `2026-10-15-research-residual-closure` 的 tasks.md 逐份核对覆盖状态
- [x] 四组子代理并行复核（10+10+10+10+10+10+10+10 份分组）提取可执行项并与台账比对
- [x] 代码级取证：`render-stability` 的 surface 钳制缺口（`context.rs:531/1144` 两处重复现算）
- [x] 确认维护者裁定：禁改区（`scripts/`、`.github/`、`android/app/build.gradle.kts`）
      四项待办「都不改，只登记」

## 二、规范增量（已完成）

- [x] 新增 `render-stability` Requirement「Surface 尺寸钳制到设备上限」
- [x] 新增 `upstream-alignment` Requirement「研究报告逐份结论登记在本规范内」
      （语料判定补齐行 33 行 + 能力裁定补齐行 37 行 + 参照面与反面做法收敛补齐行 15 行
      + 禁改区待维护者裁定项台账 4 行）
- [x] 审阅发现的两类缺口已补：索引行依据指向空表（分屏、Web 壳语料等）与
      仅存于待删报告/归档的结论（konsole/pebrel/libtsm 等 10 条语料行、
      WebSocket、ZMODEM、命令面板、远程会话、AI 集成、vi-like、文件管理器、
      结构化输出、第三方渲染实现、Terminal API 共 12 条能力行）
- [x] MCP 与无障碍朗读改判 PROHIBITED（`PROHIBITED.md` 明禁条目），
      替下先前「未声明」错裁（同 BiDi 更正口径）
- [x] RTL/BiDi 口径更正：`PROHIBITED` 无该条目，正确口径为「未声明不立项」
- [x] `docs/research/ecosystem.md`（原 `docs/persistence/ecosystem.md`，用户 commit
      `41522b67` 移入研究目录）参照面表补 mostty/hollow/con、自研 VT 组、Web 壳组
      三组漏登行；「未声明功能面统一口径」「设备性能门禁空白」两节（该文件随本轮退役）

## 三、代码实现（已完成）

- [x] `native/src/render/context.rs` 新增 `surface_extent` 纯函数
      （`#[cfg(any(target_os = "android", test))]`，与 `select_surface_format` 同口径，
      不加 `allow(dead_code)`）
- [x] `attach_surface` 与 `reconfigure_swapchain` 改用该函数，删除两处重复现算
- [x] `render/tests.rs` 新增判定表用例 `surface_extent_decision_table`
- [x] 变异验证一：去掉钳制判红（tests.rs:56 断言失败）
- [x] 变异验证二：去掉下限判红
- [x] `cargo fmt --check`、`cargo clippy --all-targets --deny warnings`、
      针对性 `cargo test --lib surface_extent` 全绿

## 四、代码重复率（jscpd 实测）

本轮用 `npx jscpd --min-lines 8 --min-tokens 60` 对 `native/src` 与 `android/app/src`
实测（仅运行工具，不改 `scripts/`）。基线：Kotlin 1.445% / Rust 1.325%；
处理后 Kotlin 1.406% / Rust 1.226%。

- [x] 生产代码三处重复已消除：
  - `render/font/font_db.rs`：`parse_fonts_xml_declared_files` 与
    `parse_fonts_xml_aliases` 各写一份「族内 font 文件名收集」，收敛为
    `family_font_files` + `fonts_xml_families_of` 两个私有助手（三个解析入口
    共用同一条遍历链）
  - `render/font/rasterization.rs`：`scaled_metric` 与 `cell_metrics` 各写一份
    「开面 + 算 scale」，收敛为 `with_font_metrics`
  - `android/app/src/test/.../BootstrapInstallerTest.kt`：三个 second-stage 用例
    各写一份 postinst 脚手架，收敛为 `seedPostinst`
- [x] 实测教训已记录：`rasterization.rs` 收敛第一版把 `extract(metrics) * scale`
    写成 `extract(metrics)`（丢缩放），既有 6 条用例（`termux_formula_*` 与三条
    光标用例）立刻判红——证明该套测试对该链路有判别力，已修复并全量复跑
- [x] 剩余重复的裁断（不修，附理由）：Kotlin 侧剩余 43 处克隆全部在 `androidTest`
  仪器化用例，重复内容是各用例的 KDoc 头与 `runBlocking` 脚手架——被测行为互不相同，
  合并会把「用例写什么」与「怎么驱动」耦合成一处；Rust 侧剩余 31 处全部在
  `#[cfg(test)]` 模块（GPU 夹具与构造样板）与 `internal.rs` 的同形参数表
  （传状态必需的形参列表，收敛成上下文档反而拖长每一次调用）

## 五、审阅循环

- [x] 第一次完整审阅（对照两份文档集逐份抽查 + 全量构建测试）
- [ ] 第二次完整审阅（对抗式：构造可破坏新规范条款与实现的输入）
- [ ] 第三次完整审阅确认无遗留

## 六、禁改区待维护者裁定（AGENTS.md：`scripts/`、`.github/`、`android/app/build.gradle.kts`）

维护者已裁定「都不改，只登记」，落地方案见 `upstream-alignment` 新 Requirement：

- [x] aislop / jscpd 门禁（TESTING.md 环境节）——已登记，未实施
- [x] rapidocr OCR 自动化（TESTING.md 环境节 + 外部基准优先级 4）——已登记，未实施
- [x] CI 产物哈希比对（vercel-labs-wterm 研究结论）——已登记，未实施
- [x] Android 15+ 16KB 页 `.so` 对齐（GlobalTechInfo-RedTerm 研究结论）——已登记，未实施
