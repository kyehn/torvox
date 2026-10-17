# 任务

## 1. 实测取证

- [x] 1.1 拉取 Unicode 18.0.0 `GraphemeBreakTest` 到 `/tmp/unicode-ucd`（不入库）。
      验证：文件头为 `# GraphemeBreakTest-18.0.0.txt`，883 行。
- [x] 1.2 写一次性探针 `native/tests/upstream_probe.rs`，分别取渲染通道与快照通道。
      验证：`cargo test --test upstream_probe` 跑通并输出分类计数。
- [x] 1.3 取实测数字：总簇数、剔除数、参与数、最长簇、两条通道互相不一致数、
      与 UAX 期望相等数、差异簇首码位穷举。
      验证：见规范「实测记录」表；探针跑完即删。
- [x] 1.4 删除探针，确认仓库无残留。
      验证：`git status` 无 `native/tests/upstream_probe.rs`。

## 2. 复核既有结论的可复核依据

- [x] 2.1 alacritty：定位期望值的产出点。
      验证：`alacritty/src/window_context.rs` 的 `write_ref_test_results` 克隆运行中的
      `Grid<Cell>` 并序列化落盘；`alacritty_terminal/tests/ref.rs` 回放时读该文件比对。
- [x] 2.2 xterm.js：确认语料里确有多参数与未实现序列，并检查有无失效期望值文件。
      验证：`t0021-CUB.in` 含 `CSI 2;3D`；`t0040-REP.in` 含 `CSI 3 b`；
      `t0031-HPB.text` 无同名 `.in` 且 `run_tests.py` 只按 `*.in` 取用例 → 无消费者。
- [x] 2.3 contour / foot / kitty / wezterm / termux / rio：逐项定位依据。
      验证：contour 21 个脚本无期望输出文件、7 个脚本以 `read` 等待人工输入；
      foot `tests/` 仅 `meson.build` 与 `test-config.c`；kitty 断言打在 C 扩展白盒钩子上；
      wezterm 快照宏来自 `k9` 依赖且置位环境变量即自动改写期望值；
      termux 断言对象是被测模拟器自身的回写与样式解码；
      rio 的 `.rgba` 确由三种外部编码器产出，但采纳它要求先实现 DESIGN 未要求的 sixel。
- [x] 2.4 esctest2：复核阻塞条件是否仍在，并区分两层。
      验证：绑定层所钉的终端源码树内 `DECRQCRA` 零命中；终端上游 master 已有分发与
      校验和计算 → 阻塞条件仍在，缺口在绑定层而非终端上游。

## 3. 规范落地

- [x] 3.1 新增「已复查结论登记在本规范内」Requirement，承载逐项结论表与实测记录表。
- [x] 3.2 在 Requirement 中固定「同一上限 MUST 只由一个具名常量表达」，
      使派生字面量不得再散落。
- [x] 3.3 `openspec validate record-upstream-corpus-verdicts --strict` 通过。
- [x] 3.4 `openspec archive` 后 `openspec validate --specs` 全绿。

## 4. 代码

- [x] 4.1 渲染路径的 `[0u32; 7]` 与 `.take(7)` 改用 `MAX_GRAPHEME_CLUSTERS - 1`。
      验证：`cargo test --workspace` 全量通过；`cargo clippy --all-targets -D warnings` 无告警。
- [x] 4.2 确认改动前后行为一致。
      验证：探针在改动**前后各跑一次**，两次输出逐项相同（1547/241/1306、最长簇 6、
      两条通道互相不一致 0、与 UAX 期望相等 922、首码位不同 384），故取数结果未变。
      补充：后随审查把 `- 1` 偏置提为具名 `MAX_GRAPHEME_EXTRA`，等价性同上。

## 5. 全量验证

- [x] 5.1 `cargo test --workspace` 全绿。
      验证：542 + 2 通过，0 失败（改动前基线相同）。
- [x] 5.2 `cargo clippy --workspace --all-targets -- -D warnings` 无告警。
- [x] 5.3 `cargo fmt --check` 无差异。
- [x] 5.4 规范文档未出现在 `git log` 的源码改动提交中，规范变更独立成步。

## 6. 附带修复

- [x] 6.1 `cargo fmt --check` 有三处既有违规（`native/build.rs`、
      `native/src/android/ffi.rs`、`native/src/render/font/shaping.rs`），非本次引入，
      但会让仓库自带门禁常红。一并格式化。
      验证：`cargo fmt --check` 无输出；格式化后 `cargo test --workspace` 仍 542 + 2 通过。
- [x] 6.2 跑完 `check-rust.nu` 后发现 `markdownlint-cli2` 在
      `docs/specification/STYLE.md` 报尾随空格（第 58 行残留空列表项），门禁在
      markdownlint 一环中断、bench 未执行。删除该空列表项（纯排版修正）。
      验证：`markdownlint-cli2 "**/*.md"` 221 文件 0 问题；三个 bench 随后跑通。
