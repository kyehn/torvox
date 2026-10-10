## Why

第一版调研只留下结论「不引入任何上游测试资产」，没有逐项依据。其中「xterm 一项」
当时被归为「断言是 xterm 自身期望值，非外部真相」——这个判断**错了**：xterm.js 的
`test/fixtures/escape_sequence_files/` 是 vttest（Thomas Dickey）→ vt100-parser
（Mark Lodato）→ xterm.js 逐代继承的语料，期望值是**从 xterm 逐帧手抄的 80×25
屏幕文本**，不是 xterm.js 自己的输出。

2026-10-12 复查把它真正下载下来并实测，得到一个此前不存在的数字，也得到一个此前
被忽略的结论。

## 实测：76 例中今天有多少例通过

一次性探针（跑完即删，不留在仓库）：对 `escape_sequence_files/` 下每个 `t*.in`，
新建 80×25 终端、灌入字节、取快照、逐行与同名 `t*.text` 比较；含 `^` 标记的用例
另行核对光标位置。

结果：**76 例中 54 例逐行与光标全对，22 例有差异。**

（探针本身也暴露了两处本仓测试工具的缺口：渲染快照时把空单元格直接跳过而非补空格，
且没有处理 vttest 的 `^` 光标标记。首轮误报 43 例差异即源于前者，不是 VT 行为差异。）

差异聚成三簇，逐簇核对 `.in` 后可以定性：

1. **格式约定（6 例）**：`^` 是 vttest 的光标标记而非屏幕字符，而部分用例同时又
   打印了字面 `^`，两者必须逐例区分——这是快照格式问题，不是 VT 行为差异。
2. **畸形/歧义序列（≥8 例：CUF、HPR、CUB、CUU、CUD、VPR、VPB、REP、reverse_wrap）**：
   逐例读 `.in` 后可见，输入是 `CSI 2;3D` 这类**参数个数不合规**的序列，或 xterm
   快照当年尚未实现的序列。典型两例：
   - `t0021-CUB` 输入 `abcdefg^[[2;3D!@`：xterm 把多出来的 `;3` 当作行移动，
     本仓按 CUB 取首参数，两者在**标准未规定处**分叉；
   - `t0040-REP` 输入 `^[[3b<`：ECMA-48 的 REP 是标准序列，xterm 快照当年未实现，
     本仓实现了，于是本仓比快照**更符合标准**。
3. **DECSTBM 与左右边距交互（7 例：t0077、t600–t603、t0076）**：xterm 按左右边距
   滚动，本仓按全宽滚动；这是真实的语义分歧，需按 DEC STD 070 逐例裁定。

## 结论

- 该语料对**格式合规**的序列是外部真相，且是九套里唯一覆盖 DEC 屏幕语义到这种
  粒度的语料，值得采纳。
- 但它记录的是「xterm 在某个历史时刻的行为」，对畸形序列记录的恰恰是**不合规**
  的行为。整包采纳等于让本仓与 xterm 的历史缺陷保持一致（REP 一例已经是本仓更
  对、快照更错）。这与「测试必须是正确性测试而非自验证」同源但方向相反：
  **整包采纳会变成「对标快照」而不是「对标标准」**。
- 故采纳口径必须是：**只采纳输入格式合规且语义唯一的用例**，畸形/歧义序列另行
  按 ECMA-48 与 DEC STD 070 写断言（那才是正确性测试）。可采纳规模与当前通过数
  一致，即 54 例起步，之后按「实测差异数归零」推进。

## 九套上游的逐项判定

| 项目 | 许可证 | 用例规模 | 期望值来源 | 判定 |
| --- | --- | --- | --- | --- |
| esctest2 | GPL-2.0 | 548 | 手写自 xterm ctlseqs 与 DEC 标准 | **阻塞**：屏幕断言全部经 `AssertScreenCharsInRectEqual` → `GetChecksumOfRect` → DECRQCRA，而本仓所钉的 ghostty `3425025e` 没有 DECRQCRA（ghostty `main` 已有）。宿主已备好可移植的 MIT harness（ghostty `test/esctest`，约 150 行 Zig，forkpty + 回填 DA/DSR/checksum），前提具备后照抄即可 |
| xterm.js | MIT | 76 | vttest 派生，从 xterm 手抄 80×25 屏幕 | **有条件采纳**：见上，54 例起步 |
| ghostty | MIT | — | 自带上述 esctest harness | 采纳其 harness 模式（而非其用例） |
| rio | MIT | — | `rio-vt/tests/sixel/` 三个由**三种不同外部编码器**产出的 `.sixel` 与对应 `.rgba` 像素（66.9 KB） | 不采纳：torvox 不实现 sixel，采纳即先实现一个 DESIGN 未要求的功能 |
| kitty | GPL-3.0 | 879 | `GraphemeBreakTest.json`（Unicode UCD，159 KB）；`screen.py` 内联的 vttest 屏幕态 | 不采纳：许可证排除；且唯一可机器消费的语料是字素分割，与 VT 无关 |
| wezterm | MIT | 409 | 48 处 `k9::snapshot!`，`K9_UPDATE_SNAPSHOTS=1` 自动改写期望值；`BidiTest.txt` 来自 Unicode UCD 但属 bidi 非 VT | 不采纳：VT 部分是教科书式自验证 |
| alacritty | MIT | 255 | `grid.json` / `alacritty.recording` 由 `--ref-test` 从运行中的 `Grid<Cell>` 序列化而来（47 MB） | 不采纳：全部自生成 |
| contour | Apache-2.0 | 21 脚本 | DEC 一致性脚本，但**无期望输出文件**，结尾 `read` 等人眼判定 | 不采纳：人眼是 oracle，无法自动化断言 |
| foot | MIT | — | 仅 `tests/test-config.c` 一个文件，无语料 | 不采纳：无资产 |

## What Changes

- 把逐项判定、实测数字与采纳口径补进本提案。
- 在 `upstream-alignment` 规范中要求：判定上游语料 MUST 实测通过数而非凭印象；
  采纳 MUST 按「输入是否格式合规」分流，畸形序列 MUST 按标准另写断言。

## Capabilities

### New Capabilities

- `upstream-alignment`：依赖与上游测试资产的对齐口径。

### Modified Capabilities

无。

## Impact

- `openspec/specs/upstream-alignment/spec.md`：新增两条采纳判定要求。
- `Cargo.toml` / `Cargo.lock`：libghostty-vt 跟进上游 master（`8953a74` → `d2036ba`，
  唯一适配点是 `on_clipboard_write` 回调签名）。
- 无运行时行为改动；探针已删除，未留任何常红的测试。
