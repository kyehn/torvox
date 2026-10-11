# 设计

## 为什么做这件事

用户给出 25 个终端类 Android 仓库，要求逐个研究是否有可参考点、检查完整、写入文档。
仓库规范 `openspec/specs/upstream-alignment/spec.md` 要求「复查某项上游资产 MUST 在本规范内
留下逐项台账」，所以这次复查的落点就是该台账。

## 亲自复核记录

子代理给出结论后，以下六条直接影响台账判定的断言由本人用命令重新核实，不采信转述：

| 断言 | 核实方式 | 实测结果 |
| --- | --- | --- |
| termux JUnit「断言是自身期望值」 | `grep -c assertLinesAre/assertLineIs` 与 `grep -c MockTerminalOutput`、外部规范 URL 统计 | 屏幕文本断言 **271 处**、`MockTerminalOutput` **3 处**、外部规范引用 16 处 → 既有台账整包判门槛 1 是未经验证的概括 |
| ghostty-android-terminal 期望值锚定外部协议 | 检索 `@Test` 计数与协议字面量 | 17 文件 / **160 个 `@Test`**；`EmulatorVtTest.java:423` 断 DSR 应答字面量 `\u001b[1;1R`、`:647` 断 XTWINOPS `\u001b[8;5;20t` → 确为协议字面量 |
| charon 的 golden 可被自身输出覆写 | 检索 `regenGoldens` 与 golden/bin 配对 | `CorpusTest.kt:26` 支持 `-Dcharon.regenGoldens` 用自身 `render()` 覆写；5 个 `.golden` 与 5 个 `.bin` 配对 → 门槛 1 与「可被实现覆写」均违反 |
| terminator 零测试但 CI 自称跑测试 | `find` 测试文件 + 读 workflow | 测试文件 **0** 个，`.github/workflows/build.yml:24-25` 仍有 `gradle test` → CI 虚假绿灯 |
| AndroidThothTerm 语料无消费者 | 全仓（含 CI 与构建脚本）检索引用 | `tests/` 共 **122 个文件 / 11 个子目录 / 864K**，全仓引用数 **0** |
| termux-kotlin-app 的净增候选 | 读 `KeyHandlerTest.java` 与 `TerminalTest.java` | `KeyHandlerTest.java:31-45` 断言 termcap 能力名到字节序列（`kb`→`\u007f`、`kB`→`\033[Z`、`kUP`→`\033[1;2A`），出处为 terminfo 规范；`TerminalTest.java:59-65` 断言越界点击按边缘钳位为 `\033[<0;1;1M` 并注明 termux-app issue 501 |

## 分流口径

沿用本规范既有的三条门槛，而非按「测试多不多」判断：

- **门槛 1（期望值须为外部真相）**：手写屏幕文本并注明规范出处 → 不违反；断言对象是自身
  回写字节流或可被 `-Dxxx.regen` 覆写的 golden → 违反。
- **门槛 3（须为净增覆盖）**：断言打在本仓不实现的组件（如上游 VT 引擎）→ 不满足。
- **无资产**：仓库无源码 / 无测试 → 结论即「无资产」，属复查结论而非遗漏。

## 刻意未做的事

- 不引入任何依赖、不 vendor 代码或测试数据、不做兼容层：本仓是应用不是库，VT 引擎槽位
  已由 `libghostty-vt` 占据。
- 不把 `docs/specification/` 当作落点：它在 AGENTS.md 的禁止修改清单内。
- 不把「目录结构相似」当作同源证据：`kosh` 与 `ReTerminal` 的 proot 树经 `diff -rq` 实测完全
  相同，确为同源；而 `on3-terminal` 是 termux-app 移植这一点**无仓内出处声明**，只作推断并标注。
