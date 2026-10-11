# 任务

## 1. 复查取证

- [x] 1.1 25 个仓库全部克隆成功，记录规模与源码构成（语言分布、单元测试文件数、VT 相关文件数）。
      验证：25/25 克隆成功，两个重名目录（`termux-monet`、`termux-app`）改名后补下。
- [x] 1.2 逐仓核实「实质」：是终端模拟器、UI 壳、环境 SDK，还是空仓库 / 只有文档 / 只有二进制。
      验证：每个仓库均实际打开文件确认，不依赖 README 自述。
- [x] 1.3 逐仓核实测试资产的期望值来源：外部真相还是自身实现输出。
      验证：抽查关键断言的形态与出处（见 1.4）。
- [x] 1.4 亲自复核影响台账的六条关键断言。
      验证：见 design.md「亲自复核记录」一节，全部以命令输出为准。

## 2. 规范

- [x] 2.1 更正既有台账中 termux `terminal-emulator` 那一行的门槛判定。
      验证：实测 `MockTerminalOutput` 3 处 vs 屏幕文本断言 271 处、外部规范引用 16 处。
- [x] 2.2 新增 25 项台账，逐条给出实质、门槛判定与可复核依据。
      验证：`openspec validate --specs` 通过；markdownlint 0 issues。

## 3. 验证

- [x] 3.1 `openspec validate --specs` 与 markdownlint 通过。
      验证：`Totals: 23 passed, 0 failed`；markdownlint 0 issues。
- [x] 3.2 仓库自带全量门禁通过。
      验证：`scripts/check-rust.nu` 退出码 0，542 + 2 测试通过。

## 4. 附带修正

- [x] 4.1 AndroidThothTerm 的孤儿语料规模以实测为准。
      验证：`tests/` 实为 122 个文件 / 11 个子目录 / 864K（全仓对 `tests/` 的引用数为 0），
      早前按两个子目录上报属漏统计，已更正。
