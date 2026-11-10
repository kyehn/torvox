# 代码健康度审查留档

本留档覆盖 `code-health` 规约要求的四类口径：重复率、代码规模、扫描命中逐条判定与
孤儿文件。所有数字均为本次实测，命令与参数写在各节内，可逐项复算。

## 审查范围与覆盖证据

- 被跟踪文件 1410 个；源码集合为 `native/src` 与 `android/app/src/main`（重复率扫描），
  规模统计另含 `native/tests` 与 `native/benches`。
- 全部数字取自已提交的树快照（`git archive <提交> | tar -x -C <目录>` 后在快照上测量），
  不取工作区状态：工作区可能同时存在他人进行中的改动，逐次测量会漂移。重复率的
  「收敛前」快照取本次收敛开始前的提交，「收敛后」快照取收敛完成后的提交。
- 参考项目快照重新拉取到本地缓存后统计：`termux/termux-app` 与
  `sylirre/ghostty-android-terminal`，两者与上一轮留档的数字相差 2 行与 43 行，
  说明口径可复现。
- 静态扫描：`semgrep` 按仓内规则集扫描全部被跟踪文件；Android `lintDebug` 跑通；
  Rust 侧逐文件统计 `unsafe`、`unwrap()`、`expect(`。

## 重复率

扫描参数：`npx jscpd@4 --min-lines 8 --min-tokens 60 --reporters json --output <目录>
native/src android/app/src/main`。

| 阶段 | 克隆处数 | 重复行 | 行占比 | token 占比 | 源文件 | 源码行 |
| --- | --- | --- | --- | --- | --- | --- |
| 收敛前 | 5 | 54 | 0.1% | 0.12% | 207 | 54818 |
| 收敛后 | 1 | 12 | 0.02% | 0.02% | 207 | 54789 |
| 当前树复算（`ff1a4ff3`） | 1 | 12 | 0.02% | 0.02% | 219 | 56971 |
| 规约留档基线 | 2 | 26 | 0.04% | — | 115 | — |

当前树复算的唯一克隆仍是 `launcher_monochrome.xml` 与 `logo_art.xml` 的同形 `pathData`
（下方「天然相似」一行已逐条判定），与收敛后同一处；源文件与源码行随代码树增长而增加，
重复率与克隆处数未变。

收敛前的五处克隆与处置：

| 位置 | 判定 | 处置 |
| --- | --- | --- |
| `native/src/android/ffi/state.rs` 的 `getGridRowsColsPacked`、`getVtBusyNanos`、`getPtySendBusyNanos` | 真重复：三处导出各自抄了一份「查注册表 → 取条目 → 读值」骨架 | 收敛为单一入口 `with_session_entry`，三处导出与原子标志读取共用 |
| `native/src/terminal/session/tests/clipboard.rs` 与 `write_backpressure.rs` 的会话夹具 | 真重复：同一段「造 MockPty → 装箱 → 建会话 → 包 `Arc<Mutex<…>>`」在多处复制 | 收敛为 `mock_pty::mock_session` 与 `mock_pty::mock_arc_session` 两个共享构造器 |
| `native/src/terminal/ghostty_terminal/tests/selection_search.rs` 的两条 ED2 放置用例 | 真重复：同一段「送图 → 取帧 → 断言一个放置 → ED2 → 取帧 → 断言放置为空」 | 收敛为共享流程 `ed2_clears_direct_kitty_placement`，提示符标记作为前置字节传入 |
| `android/app/src/main/res/drawable/launcher_monochrome.xml` 与 `logo_art.xml` | 天然相似：同形 `pathData`。Android drawable 不支持跨文件引用几何，单色版必须自带路径 | 不修，理由同上 |

与规约留档基线的偏差说明：基线记录的第二处克隆是
`native/src/terminal/ghostty_terminal/internal/mod.rs` 中 `process_query` 与
`drain_queries` 的同形形参表，该模块此后已重组，两个函数不再同时存在于同一文件，
该克隆随之消失；基线记录之后新增的代码引入了上表前四行中的三处新克隆，本次全部
收敛。源文件数从 115 增至 207 是代码树增长，与统计参数无关。基线数字不因本次处置
而改写，重复率判定以「不高于基线」为准：当前 0.02% 低于基线 0.04%。

## 代码规模

口径：源码行数（SLOC），排除空行、纯注释行、块注释、文档、配置、资源、构建产物与
第三方代码；生产与测试按路径分列。纳入扩展名 `.java` `.kt` `.rs` `.c` `.h` `.cc`
`.cpp` `.hpp` `.m` `.mm`；排除目录 `.git` `.github` `.gradle` `.idea` `build`
`target` `out` `dist` `node_modules` `generated` `third_party` `thirdparty`
`vendor` `external` `submodules` `cmake-build-debug` `.cxx` `assets`；测试判据为
路径含 `test` `tests` `androidTest` `testDebug` `testRelease` `unitTest`
`instrumentedTest` `benches` `bench` `benchmark` `benchmarks` 目录，或文件名为
`Test`/`Tests`/`TestCase`/`Spec`/`Benchmark`/`Bench`/`tests`/`*_test` 形态。

| 项目 | 生产 SLOC | 测试 SLOC | 源码合计 | 测试/生产比 | 生产文件 | 测试文件 |
| --- | --- | --- | --- | --- | --- | --- |
| 本项目 | 38731 | 39650 | 78381 | 1.02 | 184 | 251 |
| termux-app | 26637 | 2471 | 29108 | 0.09 | 181 | 23 |
| ghostty-android-terminal | 22381 | 2850 | 25231 | 0.13 | 90 | 18 |
| 两者之和 | 49018 | 5321 | 54339 | 0.11 | 271 | 41 |

本项目一行取自提交 `ff1a4ff3` 的树快照（`git archive` 后按下述脚本统计），两个参考
项目取自本地缓存的浅克隆，脚本与参数一致。

与本留档建立时的基线（提交 `e4de2d65`：生产 37829 / 测试 38979 / 合计 76808，生产
171 / 测试 247 个文件）的偏差：生产 +902、测试 +671、合计 +1573 行，生产文件 +13、
测试文件 +4。来源是此后的 36 个提交——设置页按职责拆分（外观/主题/Shell/Bootstrap/
清除数据分区）、`SelectionManager`/`FontManager`/`BootstrapManager`/`ThemeManager`/
`SelectionState`/`SessionState`/`TerminalSurfaceMath` 的独立文件化、新会话首帧按会话
记账与首帧层恢复、切后台渲染暂停的归属转场、输入法锚点契约、主界面单实例化，
以及这些改动各自带出的回归用例。两个参考项目在同口径下的复算结果与上一轮一致
（termux 26637/2471、ghostty 22381/2850），偏差只来自本仓代码树增长。

规模约束判定：`STYLE.md` 要求「代码量不得超过 ghostty-android-terminal + termux」。
生产口径 38731 低于两者生产之和 49018（占 79.0%），测试口径 39650 高于两者测试之和
5321，源码合计 78381 高于两者合计 54339（为 1.44 倍）。同一约束在不同口径下结论
相反，按 `code-health` 规约该判定为**不可判定**：口径与留档位置已如上固定，可判定
的部分是生产代码规模在参考项目之和以内，差额由测试代码占据。规约明确
MUST NOT 通过删除测试满足该约束，故本轮不以削减测试换取结论。

## 帧率与省电分支

按 `code-health` 规约「显示节拍禁止使用固定帧率和省电模式分支」核对：生产代码中
`setFrameRate`、`isPowerSaveMode`、`getRefreshRate`、`preferredDisplayModeId`
零命中。`android/app/src/main` 内与 FPS 有关的表述只出现在注释与测试常量里
（`FrameTimingStats` 的帧时长窗口说明、`KittyGraphicsOverlayTest` 的
`MIN_FPS` 下界），没有任何按固定帧率选渲染路径的分支。结论：未见帧率声明，
符合「以显示帧回调作唯一帧源、定时闭锁只作安全网」。

## 扫描命中逐条判定

| 扫描 | 命中 | 判定 | 可复核依据 |
| --- | --- | --- | --- |
| `semgrep`（仓内 54 条规则，1395 个被跟踪文件） | 0 | 通过 | 扫描退出码 0，`Findings: 0` |
| Android `lintDebug` | 0 | 通过 | `lint-results-debug.txt` 为「No issues found.」；此前的唯一命中是 `NewerVersionAvailable`（`io.mockk:mockk` 1.14.11 有 1.14.12 可用），已随依赖刷新升到 1.14.12 |
| `aislop scan --json native/src` | 1 | 不是缺陷 | 唯一命中是 `complexity/file-too-large`：`native/src/terminal/ghostty_terminal/public_api.rs` 1123 行。工具自身把该项归为 `style-policy`，即「大文件是复核压力，体积本身不是已确认缺陷」；该文件承载 VT 对外 API 的单一门面，详见下方超大文件一节 |
| `aislop scan --json`（全仓） | 1 | 不是缺陷 | 与子目录扫描同一条命中；`score` 100、`label` `Healthy`、`confirmed-defect` 0。覆盖面限制仍在：`unsupportedFiles` 164、`dominantUnsupported` 为 `Kotlin`，该工具扫不到本仓界面侧代码，故其分数不得当作全仓质量结论 |
| Rust 生产代码 `unwrap()` | 0 | 通过 | 全量统计生产路径无 `unwrap()`（测试专用文件与 `#[cfg(test)]` 模块已剔除） |
| Rust 生产代码 `expect(` | 19 | 不是缺陷 | 逐条为不变式断言：`font.rs` 的字体族计数上界（2）、`render/window.rs` 的渲染状态必然已初始化（2）、`logging.rs` 的硬编码字符串无 NUL 与日志器只装一次（4）、`glyph_cache.rs` 的容量常量大于零（6）、`theme.rs` 的默认主题必在清单中（2）、`internal/commands.rs` 的缓存必然存在、`render/cell_builder.rs` 的增量构建蕴含缓存存在、`pty.rs` 的超时值已钳位在 `i32` 范围内各 1 处。均为编译期或初始化期即可判定的事实，不存在运行时不确定输入 |
| Rust 生产代码 `unsafe` | 127 | 不是缺陷 | 口径：`native/src/**/*.rs`，剔除 `#[cfg(test)]` 模块块与测试专用文件（`tests.rs`、`*_tests.rs`、`mock_pty.rs`、`test_helpers.rs`）后，去掉注释再统计 `unsafe` 一词的出现：75 处是 JNI 导出的 `#[unsafe(no_mangle)]` ABI 标注；3 处是 `unsafe fn` 签名；45 处是 `unsafe` 块；4 处是 `unsafe extern` 块与导出签名。全部落在 FFI、系统调用与图形后端路径：`font.rs`/`input.rs`/`query.rs`/`session_ops.rs`/`window.rs`/`wake.rs`/`state.rs`/`session.rs` 的 JNI 数组读取与 `ANativeWindow_*` 调用、`logging.rs` 的日志器一次性安装、`renderer.rs`/`wgpu_backend.rs` 的 wgpu 设备创建（被 `catch_unwind` 包裹）、`ghostty_terminal/mod.rs` 的 PNG 解码器安装、`pty.rs`/`pty/child.rs` 的 `fork`/`termios`/`libc` 系统调用、`ghostty_terminal/internal/selection.rs` 的 `fmt::Formatter::set_selection`（上游把「装指定选区」标为 unsafe，其前提是两端点须为该格式化器所属终端同一活动屏的有效快照，本仓两端点取自 `absolute_point` + `grid_ref` 且其间无终端写操作，就地写明 SAFETY 依据）；逐处均有 SAFETY 依据，非缺陷 |
| Kotlin 主源码顶层类型引用 | 0 孤儿 | 通过 | 逐个文件提取顶层 `class`/`object`/`interface` 名并全仓检索，每个名字至少在自身文件内被引用 |
| 资源文件引用 | 0 孤儿 | 通过 | `layout`、`drawable`、`menu`、`xml` 目录下每个资源名至少被一处引用 |
| Rust 模块声明 | 0 孤儿 | 通过 | 每个 `mod` 声明都指向存在的文件；每个 `.rs` 文件都被某个 `mod` 声明或 Cargo 目标（`build.rs`、`benches/`、`tests/`）引用 |

## 超大文件：阈值跨语言应用不一致

阈值沿用历史变更 `split-native-oversized-modules` 确立的约 1100 行（`aislop` 的告警
线为超过 1100 行）。当前分布：

| 语言 | 超阈值文件 | 明细 |
| --- | --- | --- |
| Rust 生产源码 | 1 | `terminal/ghostty_terminal/public_api.rs` 1122 行 |
| Rust 测试源码 | 2 | `terminal/ghostty_terminal/tests/lifecycle.rs` 1189 行、`terminal/ghostty_terminal/tests/render_cache.rs` 1123 行 |
| Kotlin 生产源码 | 2 | `runtime/TerminalRuntime.kt` 5683 行、`ui/TerminalSurface.kt` 4022 行 |

判定：原生侧已按阈值拆分，界面侧仍留有两个超阈值生产文件，其中 `TerminalRuntime.kt`
的规模是原生侧最大生产文件的五倍。按 `code-health` 规约「超大文件阈值必须跨源码语言
一致应用」，本项**判定为不一致并登记为未完成**。

与本留档建立时（`e4de2d65`）的差异：界面侧原有的四个超阈值文件已消解三个——
`TerminalViewModel.kt`（2143 → 1094 行）随 `SelectionState`/`SessionState` 独立成文件，
`ui/SettingsScreen.kt`（1303 → 213 行）按外观/主题/Shell/Bootstrap/清除数据分区拆开，
`ui/TerminalSurface.kt`（4438 → 4022 行）抽出 `TerminalSurfaceMath` 顶层纯计算函数后
仍在阈值上。`render/font/tests/glyphs.rs`（1036 → 1049 行）已降至阈值下。距阈值最近的
其余文件是 `TerminalViewModel.kt` 1094 行、`render/cell_builder.rs` 1094 行、
`ui/TerminalScreen.kt` 1029 行、`ui/ModifierBar.kt` 1021 行。

逐条保留理由与后续处置方向：

| 文件 | 关注面 | 处置方向 |
| --- | --- | --- |
| `terminal/ghostty_terminal/public_api.rs` | VT 对外 API 的单一门面：构造与生命周期、VT 写入与帧取回、快照与查询、键鼠编码 | 按上述四类独立职责切分，切分后门面只留转发 |
| `runtime/TerminalRuntime.kt` | 会话生命周期、渲染线程调度、看门狗、通知、剪贴板、响铃、主题、字体、输入法、选区、滚动、前台服务 | 按关注面切分为若干独立文件，运行时门面只留编排 |
| `ui/TerminalSurface.kt` | 触摸与手势映射、绘制、选区、滚动、输入法插入 | 绘制与输入两条线分开 |
| `terminal/ghostty_terminal/tests/lifecycle.rs`、`tests/render_cache.rs` | 测试文件，本身不受生产源码规模约束 | 不处置 |

拆分 MUST 依据彼此独立的职责边界，MUST NOT 按行数机械切分，也 MUST NOT 制造仅用一次的
空壳抽象。

## 明确不作处置的项

| 候选 | 不作处置的理由 |
| --- | --- |
| 合并 `launcher_monochrome.xml` 与 `logo_art.xml` 的同形 `pathData` | Android drawable 不支持跨文件引用几何，单色版必须自带路径 |
| 以削减测试换取规模约束达标 | `code-health` 规约明令 MUST NOT 通过删除测试满足约束 |
| 改动 `app/build.gradle.kts` 升级 mockk | 构建脚本属受保护文件；lint 的唯一命中不是缺陷 |
| 拆分生产代码以压低合计行数 | 生产口径已在参考项目之和以内；合计口径的差额来自测试代码，拆分生产代码不改变结论 |
| 把重复率与规模度量接入门禁 | `scripts/check.nu` 与持续集成工作流属受保护文件，接入方式由维护者裁定 |

## 复现方法

重复率：在 `nix develop` 内把待测树导出为快照（`git archive <提交> | tar -x -C <目录>`），
对本节「重复率」给出的命令替换两个源码路径，读 `--reporters json` 输出的
`statistics.total`（`clones`、`duplicatedLines`、`percentage`、`sources`）与逐处克隆明细。

规模：把下述脚本存为 `sloc.py`，对三个项目根目录各执行一次，比较输出的
`production`、`test`、`total` 三档。本仓的根目录用同一份快照，参考项目需先浅克隆到
本地缓存再统计。

```python
#!/usr/bin/env python3
"""统一源码行数（SLOC）口径：排除空行、纯注释行、块注释、文档、配置、资源、
构建产物与第三方代码；生产与测试按路径分列。"""

import os
import re
import sys

SOURCE_EXTENSIONS = {".java", ".kt", ".rs", ".c", ".h", ".cc", ".cpp", ".hpp", ".m", ".mm"}
EXCLUDED_DIRECTORIES = {
    ".git", ".github", ".gradle", ".idea", "build", "target", "out", "dist",
    "node_modules", "generated", "third_party", "thirdparty", "vendor",
    "external", "submodules", "cmake-build-debug", ".cxx", "assets",
}
TEST_DIRECTORIES = {
    "test", "tests", "androidTest", "testDebug", "testRelease", "unitTest",
    "instrumentedTest", "benches", "bench", "benchmark", "benchmarks",
}
TEST_FILE_PATTERN = re.compile(
    r"(^|/)(Test|Tests|TestCase|Spec|Benchmark|Bench)\.[^.]+$"
    r"|(_test|Tests|Test)\.(rs|kt|java)$"
    r"|(Test|Tests|IT|TestCase)\.(kt|java)$"
    r"|(^|/)tests\.(rs)$"
)


def is_test_path(relative_path):
    parts = relative_path.split(os.sep)
    if any(part in TEST_DIRECTORIES for part in parts[:-1]):
        return True
    return bool(TEST_FILE_PATTERN.search(relative_path))


def strip_comments(content, nested_block_comments):
    """返回去掉注释与字符串字面量后的代码行列表。"""
    lines = []
    in_block_comment = False
    block_depth = 0
    for raw_line in content.splitlines():
        result = []
        index = 0
        length = len(raw_line)
        while index < length:
            character = raw_line[index]
            if in_block_comment:
                if raw_line.startswith("*/", index):
                    in_block_comment = False
                    block_depth = 0
                    index += 2
                    continue
                if nested_block_comments and raw_line.startswith("/*", index):
                    block_depth += 1
                    index += 2
                    continue
                index += 1
                continue
            if character == "/" and index + 1 < length:
                following = raw_line[index + 1]
                if following == "/":
                    break
                if following == "*":
                    in_block_comment = True
                    block_depth = 1
                    index += 2
                    continue
            if character == '"':
                index += 1
                while index < length:
                    if raw_line[index] == "\\":
                        index += 2
                        continue
                    if raw_line[index] == '"':
                        index += 1
                        break
                    index += 1
                result.append('""')
                continue
            if character == "'":
                # Rust 的生命周期（'a）不是字符字面量：只有形如 'x' 或 '\n' 的
                # 才按字面量处理，其余按普通代码字符跳过。
                match = re.match(r"'(\\.|[^\\'])'", raw_line[index:])
                if match:
                    index += len(match.group(0))
                    result.append("''")
                    continue
            result.append(character)
            index += 1
        lines.append("".join(result))
    return lines


def count_project(root):
    production = 0
    test = 0
    production_files = 0
    test_files = 0
    for directory, subdirectories, file_names in os.walk(root):
        subdirectories[:] = [
            name for name in subdirectories if name not in EXCLUDED_DIRECTORIES
        ]
        for file_name in file_names:
            extension = os.path.splitext(file_name)[1]
            if extension not in SOURCE_EXTENSIONS:
                continue
            full_path = os.path.join(directory, file_name)
            relative_path = os.path.relpath(full_path, root)
            try:
                with open(full_path, "r", encoding="utf-8", errors="replace") as handle:
                    content = handle.read()
            except OSError:
                continue
            code_lines = strip_comments(content, extension == ".rs")
            counted = sum(1 for line in code_lines if line.strip())
            if is_test_path(relative_path):
                test += counted
                test_files += 1
            else:
                production += counted
                production_files += 1
    return production, test, production_files, test_files


def main():
    for root in sys.argv[1:]:
        production, test, production_files, test_files = count_project(root)
        print(
            f"{root}\tproduction={production}\ttest={test}"
            f"\ttotal={production + test}\tproduction_files={production_files}"
            f"\ttest_files={test_files}"
        )


if __name__ == "__main__":
    main()
```
