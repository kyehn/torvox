# 代码健康度审计留档

本留档是 `code-health` 能力的唯一审计落点：重复率、代码规模口径、静态形状扫描、依赖
活跃度四类实测数字与逐处判定都写在此处。最新采样日期 2026-11-27；测量对象是复算当时的
已提交树快照（`git archive HEAD` 展开到独立目录），不含工作区未提交改动，故结果可复算。
2026-11-24 的首轮基线保留在各表对照栏，偏差逐项写明来源。

## 重复率扫描

- 工具：`jscpd` 4.0.5（`npx --yes jscpd@4.0.5`）。
- 参数：`--min-lines 8 --min-tokens 60`（片段少于 8 行或 60 token 不计）。
- 范围：生产口径为 `native/src` 与 `android/app/src/main`；全量口径为 `native/src` 与
  `android/app/src`。
- 语义：同一语言、同一 token 序列、连续行且长度达标即计为 clone。

| 口径 | sources | lines | tokens | clones | duplicatedLines | 重复率 |
| --- | --- | --- | --- | --- | --- | --- |
| 生产 | 267 | 69185 | 525781 | 3 | 30 | 0.04% |
| 全量 | 531 | 105243 | 808208 | 26 | 257 | 0.24% |

复算轮次：2026-11-27（同一组参数、同一条命令）。克隆集合与首轮基线完全相同，
`lines`/`tokens` 随代码树变化（生产 68916 → 69140 → 69202 → 69185 行、全量 104924 →
105230 → 105296 → 105243 行；第 3、4 轮之间代码树有其他提交，行数回落）。

生产口径三处命中的判定：

1. `SettingsBootstrapSection.kt` 与 `SettingsShellSection.kt` 的同形包声明与 import 段。
   两个界面文件各自按需 import 同一组 Compose 与项目符号，合并只能靠通配导入或门面对象，
   换来的是可读性与静态检查退化，违 `STYLE.md` 的一次性抽象禁令。
2. `SettingsAppearanceSection.kt` 与 `SettingsLogLevelSection.kt` 的同形 import 段，理由同上。
3. `res/drawable/launcher_monochrome.xml` 与 `res/drawable/logo_art.xml` 的 `<vector>` 头与
   `<group>` 变换。Android drawable 不支持跨文件引用几何，单色版必须自带路径。

全量口径比生产口径多出的 23 处命中，判定为：

- 22 处是跨文件的同形包声明与 import 段。其中 21 处在测试文件之间，1 处
  （`SettingsDataStoreProvider.kt` 与 `SettingsRepositoryTest.kt`）一端是生产文件的头部、
  另一端是测试文件的同形头部；`HardKeyboardMultipleArrivalTest.kt` 与
  `StickyCtrlInterruptInstrumentedTest.kt`、`RenderPauseSemanticsTest.kt` 与
  `VtCorrectnessInstrumentedTest.kt` 两组的片段尾部各带本用例自己的说明注释，被判为同一
  类 import 样板。各文件按需 import 同一组 JUnit、Compose 与测试 API，合并同样只得到
  只为转发符号的中间层。
- 1 处是 `TermuxPropertiesParserTest.kt` 内两段结构相同的测试数据（坏对象只丢一项与相邻
  用例的同形断言段）。两用例断言的键不同，抽公共夹具会掩盖各自的不变量。

结论：生产口径无真重复，未做代码改动。生产口径三处命中的文件对与首轮基线完全相同，
重复率由 0.05% 降到 0.04% 是分母增长所致，不是新增或消除克隆。

复算期间曾出现第 4 处生产口径克隆：`native/src/terminal/pty/tests.rs` 内
`chdir_changes_working_directory` 与 `missing_working_directory_falls_back_to_root`
两处用例各自抄了一份「建 `ShellEnv` → 起 shell → 非阻塞主端 → 写命令」夹具。
按本规范「同一夹具在两处以上复制时 MUST 收敛为一处共享实现」，该夹具已收敛为
`spawn_shell_and_write`（工作目录生效、工作目录缺失回退、PATH 前缀三处共用），
调用点各自的断言强度不变；收敛后生产口径回到 3 处克隆，全量口径回到 26 处。

## 代码规模口径

统一口径：排除空行、纯注释行、块注释、字符串字面量、资源文件、构建产物与第三方代码；
生产与测试按路径分列。统计脚本全文如下，重跑命令为
`python3 sloc.py <仓库根>`。

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

| 仓库 | 生产 | 测试 | 合计 |
| --- | --- | --- | --- |
| torvox | 41834 | 50734 | 92568 |
| termux-app | 26637 | 2471 | 29108 |
| ghostty-android-terminal | 22381 | 2850 | 25231 |
| 两个参考项目生产之和 | 49018 | 5321 | 54339 |

判定：生产口径 41834 未越界（不超过 49018），满足 `STYLE.md` 的代码量约束；测试口径
50734 远超两个参考项目测试之和 5321，源码合计口径同样越界。按 `code-health`「不同口径
导出相反结论」的条款，该项判定为**不可判定**，并以生产口径数字作为后续复算基线。
不得以删测试的方式满足规模约束。

参考项目取自 `termux-app`（v0.119.0-beta.3 分支）与 `ghostty-android-terminal` 的浅克隆，
与 `docs/specification/DESIGN.md` 点名的两个参考项目一致。

## 静态形状扫描

- semgrep：`semgrep scan --config .semgrep/rules.yml --json --metrics=off
  --disable-version-check .`，55 条规则扫过 1693 个文件，**0 findings、0 errors**。
  扫描文件数随代码树增长（首轮 1691 → 复算 1693）；2026-11-27 复算仍为 1693 个文件、
  0 findings、0 errors。
  扫描须在 git 仓库内执行并配置 `origin` 远端，否则规则集里的远端引用无法解析。
- aislop 0.23.0：`npx --yes aislop@latest scan --json native/src`，`score` 100、label
  `Healthy`、`errors` 0、`warnings` 0、`coverage.supportedFiles` 116、`unsupportedFiles` 0。
  上一轮登记的 `complexity/file-too-large` 告警（`render/cell_builder.rs`）随测试体拆到
  `render/cell_builder/tests/` 后消失，告警归零。按 `code-health`「超大文件阈值必须跨源码
  语言一致应用」，本仓不得为凑行数阈值拆分生产源码文件。该工具扫不到 Kotlin 源码，只能
  作为 Rust 侧的门禁。2026-11-27 复算仍为 score 100、Healthy、0 errors、0 warnings、
  `supportedFiles` 116、`unsupportedFiles` 0。
- Rust 生产源码形状：剔除测试专用文件（`*_tests.rs`、`tests.rs`、`test_helpers.rs`、
  `mock_pty.rs` 与 `tests/` 目录）、文件内 `#[cfg(test)]` 项、注释与字符串字面量后，
  `unwrap()` 0 处、`expect(` 23 处、`panic!` 5 处、`unsafe` 132 处（unsafe block 45、
  unsafe fn 3、unsafe extern 4、unsafe impl 0）、`todo!`/`unimplemented!` 0 处。
  计数口径：`unsafe` 统计代码位置的关键字出现次数，细分按语法形态计数，
  `unsafe extern "C" fn` 同时计入 extern 与 fn 两项。

逐处判定：

- 23 处 `expect` 全是不变量断言，每处带解释性注释：日志配额非零与限流闸已初始化
  （`slow_log.rs` 2）、回滚余量已夹到合法区间（`terminal/pty.rs`）、默认主题必在主题清单内
  （`terminal/theme.rs` 2）、命令快照缓存必然存在（`ghostty_terminal/internal/commands.rs`）、
  增量渲染蕴含缓存存在（`render/cell_builder.rs`）、可复用槽位非空
  （`render/context/gpu.rs`）、字形缓存容量常量大于零（`render/font/glyph_cache.rs` 6）、
  `CString` 不含内嵌零字节且日志器未重复安装（`android/logging.rs` 4）、u16 线字段计数成立
  （`android/ffi/font.rs` 2）、渲染状态已初始化（`android/ffi/render/window.rs` 3）。
  上述清单逐文件相加为 23，与总数一致。`terminal/session/mod.rs` 的
  `output_tx send must succeed` 位于 `#[cfg(test)]` 的 `inject_output_for_test` 内，
  不属生产口径，不计入本条。
- 5 处 `panic!` 是「失败即崩、不掩盖错误」的设计决策：alacritty 主题解析失败
  （`terminal/theme.rs`）、VT 线程卡死与快照超时（`ghostty_terminal/public_api/frames.rs`）、
  GPU 初始化失败（`render/context/gpu.rs`）、gpu-acquire 工作线程创建失败
  （`render/pass/acquire.rs`）。
- 132 处 `unsafe` 集中在 FFI 与 wgpu 回调边界（`android/ffi/*`、`terminal/pty.rs`、
  `render/*`）。JNI 导出统一经 `jni_export_guard!`（`native/src/android/ffi.rs`）把 panic
  转成 Java `RuntimeException`，semgrep 规则 `jni-export-must-use-panic-guard` 覆盖该约束，
  本轮扫描未报缺口。

形状偏差来源（相对 2026-11-24 首轮基线，基线未被改写）：

- 2026-11-27 复算（第 3、4 轮同一日、两次快照）：`unwrap` 0、`expect` 23、`panic!` 5、
  `todo!`/`unimplemented!` 0、`unsafe` 132（block 45、fn 3、extern 4、impl 0），与上表
  逐项一致，无新增形状偏差。
  同一扫描口径在第 2 轮快照上重放得 `unsafe` 131（block 44），差额来自第 2 轮之后落地的
  `terminal/pty.rs` 工作目录回退修复新增的 `unsafe { libc::chdir(...) }`，与上述归因一致。

- `unwrap` 0、`expect` 23、`panic!` 5、`todo!`/`unimplemented!` 0 与首轮一致。
- `unsafe` 134 → 131：减少的三处来自 `android/logging.rs`（−1）、
  `terminal/ghostty_terminal/internal/selection.rs` 整文件移除（−1）与
  `terminal/ghostty_terminal/mod.rs`（−1）。
- `unsafe` 131 → 132（block 44 → 45）：工作目录缺失回退到 `/` 的修复在 fork 与 exec
  之间新增 `unsafe { libc::chdir(c"/".as_ptr()) }`，带 SAFETY 注释（静态 NUL 结尾路径、
  fork 后 `chdir` 异步信号安全）。
- 首轮留档的「unsafe block 129、unsafe extern 2」是「总数减去 fn 与 extern」的残差口径，
  与本轮「按语法形态逐处计数」不可直接比较；两者 `unsafe` 总数 134 一致，基线未被改写。

显示节拍核实：`setFrameRate`、`getRefreshRate`、`preferredDisplayModeId` 在生产代码零命中；
`isPowerSaveMode` 只有 `TerminalApp.kt` 的 `powerPreferenceFor` 一处，用于给 wgpu 适配器选
功耗偏好（省电时选集显），不决定帧率或渲染路径，另有 `TerminalAppPowerPreferenceTest`
覆盖该判定。符合 `code-health`「显示节拍禁止使用固定帧率和省电模式分支」。

## 依赖活跃度

采信口径：累计下载量 ≥ 100 万、近 90 天下载量 ≥ 10 万，且最近发布在 12 个月内；生态基线
类型（仅作类型边界、无独立逻辑）可豁免「最近发布」一项，但须写明豁免理由。crates.io
公开下载量，Maven Central 与 Google Maven 只公开发布元数据，本仓据后者判定发布节奏。

crates.io 直接依赖（采样于 2026-11-27）：

| crate | 累计下载 | 近 90 天 | 最近发布 | 判定 |
| --- | --- | --- | --- | --- |
| base64 | 2080383848 | 585216277 | 2026-08-04 | 保留 |
| bytemuck | 505142979 | 156368405 | 2026-07-19 | 保留 |
| cosmic-text | 13467890 | 5707278 | 2026-04-22 | 保留 |
| flume | 308820363 | 89451182 | 2025-12-08 | 保留 |
| foldhash | 899482684 | 377829161 | 2025-08-23 | 豁免（生态基线） |
| fontdb | 50280803 | 19759753 | 2026-07-29 | 保留 |
| governor | 98364161 | 24920673 | 2025-12-16 | 保留 |
| pollster | 53911038 | 25242619 | 2026-11-18 | 保留 |
| etagere | 11226202 | 5027553 | 2026-03-18 | 保留 |
| jni | 264507085 | 93802199 | 2026-03-16 | 保留 |
| libc | 2072530894 | 523539462 | 2026-10-16 | 保留 |
| log | 1610768868 | 425699321 | 2026-10-30 | 保留 |
| lru | 468401871 | 143080878 | 2026-11-23 | 保留 |
| nix | 1028484497 | 271285662 | 2026-11-17 | 保留 |
| parking_lot | 1333444596 | 305478955 | 2026-10-21 | 保留 |
| raw-window-handle | 145123537 | 50062782 | 2024-05-17 | 豁免（生态基线） |
| roxmltree | 116366930 | 47804575 | 2026-10-24 | 保留 |
| serde | 1791729068 | 465148212 | 2026-07-18 | 保留 |
| serde_json | 1699368670 | 468402314 | 2026-10-11 | 保留 |
| shell-words | 189784506 | 51383984 | 2025-12-10 | 保留 |
| swash | 20902468 | 9114554 | 2026-07-17 | 保留 |
| thiserror | 1957379631 | 575176741 | 2026-09-23 | 保留 |
| toml | 1200918637 | 343148309 | 2026-11-25 | 保留 |
| unicode-script | 68923342 | 26758342 | 2026-11-02 | 保留 |
| wgpu | 55128180 | 22074635 | 2026-11-12 | 保留 |
| wgpu-types | 57470177 | 23095186 | 2026-11-12 | 保留 |
| criterion（dev） | 354540550 | 86175706 | 2026-02-04 | 保留 |
| proptest（dev） | 262370960 | 85453517 | 2026-10-10 | 保留 |
| shuttle（dev） | 8079469 | 2833695 | 2026-11-05 | 保留 |
| tempfile（dev） | 1045968666 | 275567543 | 2026-03-11 | 保留 |

`libghostty-vt` 是 git 依赖（钉住上游 master rev），不走 registry 发布节奏，不适用下载量
口径。

2026-11-27 再采一轮：30 个 crate 的最新版本逐项未变，下载量随日累计自然增长；唯一的新发布
是 `toml` 1.2.3+spec-1.1.0（2026-11-25），本仓 `native/Cargo.lock` 已钉在该版本，发布日随
本表更新为 2026-11-25。

两项豁免的理由：`foldhash` 与 `raw-window-handle` 只提供哈希函数与窗口句柄的类型边界，
不承担独立逻辑，且被 wgpu 与窗口生态广泛依赖；两者下载量分别 9.0 亿与 1.5 亿，近 90 天
分别 3.8 亿与 5000 万，维护停滞不等于使用面窄。

Maven 与 Google 侧坐标（45 个 Gradle 直接依赖，加 compose-bom 与 5 个 lint 插件坐标，
共 51 个，采样同日）：

| 坐标 | 仓库 | 最新版本 | 最近发布 | 判定 |
| --- | --- | --- | --- | --- |
| `androidx.compose:compose-bom` | google | 2026.11.01 | 2026-11-19 | 通过 |
| `androidx.core:core-ktx` | google | 1.20.0-alpha01 | 2026-11-19 | 通过 |
| `androidx.core:core-splashscreen` | google | 1.2.0 | 2025-11-05 | 超期，见下 |
| `org.jetbrains.kotlinx:kotlinx-collections-immutable` | central | 0.5.2 | 2026-08-28 | 通过 |
| `androidx.lifecycle:lifecycle-runtime-ktx` | google | 2.12.0-alpha06 | 2026-11-19 | 通过 |
| `androidx.lifecycle:lifecycle-viewmodel-compose` | google | 2.12.0-alpha06 | 2026-11-19 | 通过 |
| `androidx.lifecycle:lifecycle-runtime-compose` | google | 2.12.0-alpha06 | 2026-11-19 | 通过 |
| `androidx.activity:activity-compose` | google | 1.14.0-alpha03 | 2026-09-23 | 通过 |
| `androidx.compose.ui:ui` | google | 1.14.0-alpha01 | 2026-11-19 | 通过 |
| `androidx.compose.ui:ui-graphics` | google | 1.14.0-alpha01 | 2026-11-19 | 通过 |
| `androidx.compose.ui:ui-test-junit4` | google | 1.14.0-alpha01 | 2026-11-19 | 通过 |
| `androidx.compose.ui:ui-test-manifest` | google | 1.14.0-alpha01 | 2026-11-19 | 通过 |
| `androidx.compose.material3:material3` | google | 1.6.0-alpha01 | 2026-11-19 | 通过 |
| `androidx.compose.material:material-icons-extended` | google | 1.7.8 | 2025-02-12 | 超期，见下 |
| `androidx.compose.foundation:foundation` | google | 1.14.0-alpha01 | 2026-11-19 | 通过 |
| `com.google.dagger:hilt-android` | central | 2.60.1 | 2026-07-06 | 通过 |
| `com.google.dagger:hilt-android-compiler` | central | 2.60.1 | 2026-07-06 | 通过 |
| `com.google.dagger:hilt-android-testing` | central | 2.60.1 | 2026-07-06 | 通过 |
| `androidx.hilt:hilt-lifecycle-viewmodel-compose` | google | 1.4.0 | 2026-07-01 | 通过 |
| `androidx.datastore:datastore-preferences` | google | 1.3.0 | 2026-10-21 | 通过 |
| `com.jakewharton.timber:timber` | central | 5.0.1 | 2021-08-13 | 超期，见下 |
| `com.squareup.okhttp3:okhttp` | central | 5.5.1 | 2026-10-22 | 通过 |
| `com.squareup.okhttp3:mockwebserver3` | central | 5.5.1 | 2026-10-22 | 通过 |
| `com.squareup.okhttp3:okhttp-tls` | central | 5.5.1 | 2026-10-22 | 通过 |
| `com.squareup.leakcanary:leakcanary-android` | central | 3.0-alpha-9 | 2026-06-24 | 通过 |
| `junit:junit` | central | 4.13.2 | 2021-02-13 | 超期，见下 |
| `io.mockk:mockk` | central | 1.14.12 | 2026-11-07 | 通过 |
| `app.cash.turbine:turbine` | central | 1.2.1 | 2025-06-11 | 超期，见下 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-test` | central | 1.11.0 | 2026-05-08 | 通过 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-android` | central | 1.11.0 | 2026-05-08 | 通过 |
| `org.jetbrains.kotlinx:kotlinx-serialization-json` | central | 1.13.0 | 2026-11-11 | 通过 |
| `androidx.profileinstaller:profileinstaller` | google | 1.4.1 | 2024-10-02 | 超期，见下 |
| `org.robolectric:robolectric` | central | 4.18.1 | 2026-11-18 | 通过 |
| `androidx.test:core` | google | 1.8.0-alpha04 | 2026-11-11 | 通过 |
| `androidx.test.ext:junit` | google | 1.4.0-alpha04 | 2026-11-11 | 通过 |
| `androidx.test.espresso:espresso-core` | google | 3.8.0-alpha04 | 2026-11-11 | 通过 |
| `androidx.test.espresso:espresso-contrib` | google | 3.8.0-alpha04 | 2026-11-11 | 通过 |
| `androidx.test.espresso:espresso-intents` | google | 3.8.0-alpha04 | 2026-11-11 | 通过 |
| `androidx.test.uiautomator:uiautomator` | google | 2.4.1 | 2026-11-04 | 通过 |
| `androidx.test:runner` | google | 1.8.0-alpha04 | 2026-11-11 | 通过 |
| `androidx.test:rules` | google | 1.8.0-alpha04 | 2026-11-11 | 通过 |
| `io.cucumber:cucumber-android` | central | 7.19.0 | 2026-10-13 | 通过 |
| `androidx.benchmark:benchmark-macro-junit4` | google | 1.6.0-alpha02 | 2026-11-18 | 通过 |
| `com.ms-square:debugoverlay` | central | 2.7.1 | 2026-10-09 | 通过 |
| `com.slack.lint.compose:compose-lint-checks` | central | 1.6.1 | 2026-10-07 | 通过 |
| `com.slack.lint:slack-lint-checks` | central | 0.11.1 | 2025-10-10 | 超期，见下 |
| `dev.detekt:detekt-rules-ktlint-wrapper` | central | 2.0.0-alpha.6 | 2026-08-04 | 通过 |
| `dev.detekt:detekt-rules-libraries` | central | 2.0.0-alpha.6 | 2026-08-04 | 通过 |
| `dev.detekt:detekt-rules-ruleauthors` | central | 2.0.0-alpha.6 | 2026-08-04 | 通过 |
| `io.github.detekt.sarif4k:sarif4k` | central | 0.8.0 | 2026-08-25 | 通过 |
| `io.github.oshai:kotlin-logging` | central | 8.0.7 | 2026-11-10 | 通过 |

七处超期的逐项结论：

- `com.jakewharton.timber:timber` 5.0.1（2021-08-13）：`docs/specification/DESIGN.md` 明文
  要求使用 Timber，本仓只用它包一层 logcat 树。规范属受保护文件，审查不得单方面推翻，
  故登记为**待维护者裁定**的冲突项：若裁定移除，需在 Kotlin 侧直接写 `android.util.Log`；
  若裁定保留，本项按生态基线豁免。替换方案的实测代价已记录在提交 `revert(app): 恢复
  Timber 输出通道并登记依赖口径冲突`。
- `junit:junit` 4.13.2（2021-02-13）：JUnit 4 是 Android 测试运行器的事实基线，只承担
  测试框架边界；无新版本可升，退出方案为改接 JUnit 5 与新运行器，收益不足。
- `app.cash.turbine:turbine` 1.2.1（2025-06-11）：上游无更新的稳定版可换，承担 Flow 断言
  的测试侧能力，退出即失去该断言手段。
- `com.slack.lint:slack-lint-checks` 0.11.1（2025-10-10）：detekt 的 lint 规则插件基线，
  版本随 detekt 2.0.0-alpha.6 生态联动。
- `androidx.core:core-splashscreen` 1.2.0（2025-11-05）、`androidx.compose.material:
  material-icons-extended` 1.7.8（2025-02-12）、`androidx.profileinstaller:profileinstaller`
  1.4.1（2024-10-02）：AndroidX 与 Compose 构件，按上游发布节奏，版本由 compose-bom 与
  目标 SDK 对齐，本仓不承担其发布节奏。

本轮重跑 51 个坐标：最新版本与 2026-11-24 采样逐项一致，无新版本；发布日按该版本
`.pom` 的 HTTP `Last-Modified` 复核，其中 `kotlinx-coroutines-test` 与
`kotlinx-coroutines-android` 读到 2026-05-07（中央仓库镜像上传时刻），首轮记 2026-05-08，
两者指向同一发布批次，不改变判定。2026-11-27 再读同一批坐标，版本号逐项未变，两个协程坐标
仍读到 2026-05-07，其余坐标的发布日与表中一致。

## 禁改区待维护者裁定项台账

规范要求而现行门禁缺席、或门禁工具覆盖面小于代码树的项，全部落在 AGENTS.md 禁改区
（`scripts/`、`.github/`、`android/app/build.gradle.kts`）。维护者已裁定「都不改，只登记」，
故本表只登记事实与验收判据，未经维护者明确同意不实施。

| 项 | 规范依据 | 落地方案（未实施） | 当次实测基线 |
| --- | --- | --- | --- |
| aislop 与 jscpd 代码检查门禁 | `docs/specification/TESTING.md` 环境节 | `scripts/check.nu` 与 CI 增 `npx aislop@latest scan` 与 `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步 | aislop：score 100、Healthy、0 errors、0 warnings、supportedFiles 116；jscpd：生产口径 3 处克隆 / 30 行 / 0.04% |
| rapidocr OCR 自动化 | `docs/specification/TESTING.md` 环境节 | `scripts/` 增 OCR 判定脚本并在 CI 模拟器步骤调用 | 未实跑（本轮无截图产物） |
| CI 产物哈希比对 | `vercel-labs-wterm` 研究结论 | CI 增产物 sha256 记录步骤 | CI 现无产物哈希步骤 |
| Android 15+ 16KB 页 `.so` 对齐 | `GlobalTechInfo-RedTerm` 研究结论 | `android/app/build.gradle.kts` 按 AGP 16KB 对齐指南调整打包 | `android/app/build.gradle.kts` 现无 16KB 对齐配置 |
| `cargo-nextest` / `cargo-mutants` / `cargo-tarpaulin` 测试工具链 | `rust-dependency-survey-2026` 研究结论 | `scripts/check.nu` 的 `cargo test --no-fail-fast` 改用 nextest 运行；变异测试与覆盖率阈值另配 CI 步骤 | 脚本现用 `cargo test --no-fail-fast`，无 nextest、mutants、tarpaulin |
| detekt 对仪器化测试源集的覆盖面 | `code-health`「规范声明的静态检查工具必须与验证门禁一致」 | `scripts/check.nu` 的 `detekt` 改列 `detektDebugAndroidTest`，使仪器化源集与生产、单测源集同受 detekt 规则约束 | 脚本只执行 `detekt`；Gradle 已提供 `detektDebugAndroidTest` 任务，仪器化源集当前不在 detekt 覆盖面内 |

验收判据：各项接入后，基线数字 MUST NOT 上升；aislop 的 `confirmed-defect` 与 `errors`
MUST 维持 0，`style-policy` 类命中 MUST NOT 阻断构建；jscpd 的重复率 MUST NOT 高于
当次基线，新增克隆 MUST 逐处给出不修的具体理由。

## 清洁运行计数

本留档的复算轮次按同一组命令在已提交树快照上重复执行，每轮覆盖重复率、代码规模口径、
静态形状扫描与依赖事实复算。当前计数为 **5**（第 1 轮 2026-11-24，第 2 轮 2026-11-26，
第 3、4、5 轮 2026-11-27），已达「连续清洁运行计数 ≥ 5」；每轮的实测数字 MUST 与本留档
各表的基线逐项对照，出现偏差时 MUST 写明偏差来源，不得直接改写基线掩盖偏差。

第 3、4、5 轮（2026-11-27）的对照结果：克隆集合、重复率、静态形状扫描、依赖版本判定与
显示节拍核实逐项与基线一致；偏差只有代码树变化带来的行数与 SLOC 浮动，已在对应表内
写明轨迹；依赖侧唯一变化是 `toml` 1.2.3+spec-1.1.0 于 2026-11-25 发布，判定不变，
其后两轮无新版本发布。第 5 轮在第 4 轮提交后的同一棵树上复算（期间无新提交），
四类口径与第 4 轮逐项相同，依赖采样另取当日新值。

## vendor 与硬编码核对

- 仓内没有 `vendor/`、`third_party/` 目录，没有 `.gitmodules` 子模块，也没有上游 Zig 源码
  副本；上游 Ghostty 的 Zig 源码由 `libghostty-vt-sys` 的构建脚本在编译期拉到
  `native/target/` 下，不进入版本库。
- 硬编码集中在协议常量与设备口径（终端网格尺寸、图集尺寸、PTY 排空期限、轮询间隔、
  超时上限），各自有规范或上游依据；用户侧可配面由 `termux.properties` 覆盖。

## 复现方法

```sh
git archive HEAD | tar -x -C /tmp/audit-head
git -C /tmp/audit-head init -q
git -C /tmp/audit-head remote add origin https://github.com/kyehn/torvox
git -C /tmp/audit-head add -A
git -C /tmp/audit-head -c user.email=audit@local -c user.name=audit commit -qm snapshot
cd /tmp/audit-head
npx --yes jscpd@4.0.5 --min-lines 8 --min-tokens 60 --reporters json \
    --output /tmp/jscpd-prod native/src android/app/src/main
npx --yes jscpd@4.0.5 --min-lines 8 --min-tokens 60 --reporters json \
    --output /tmp/jscpd-all native/src android/app/src
npx --yes aislop@latest scan --json native/src
semgrep scan --config .semgrep/rules.yml --json --metrics=off --disable-version-check .
python3 sloc.py /tmp/audit-head
```

Rust 形状扫描另按上文口径执行：剔除测试专用文件（`*_tests.rs`、`tests.rs`、
`test_helpers.rs`、`mock_pty.rs` 与 `tests/` 目录）、文件内 `#[cfg(test)]` 项、注释与字符串
字面量后，再统计 `unwrap`/`expect`/`panic!`/`unsafe`/`todo!`/`unimplemented!` 的命中。
第 3 轮起该扫描由下列脚本执行（`python3 shape_scan.py <仓库根>/native/src`），其输出字段
与上文「Rust 生产源码形状」一条逐项对应：

```python
#!/usr/bin/env python3
"""Rust 生产源码形状扫描：剔除测试专用文件、文件内 #[cfg(test)] 项、注释与字符串字面量，
统计 unwrap/expect/panic!/unsafe/todo!/unimplemented! 的命中。"""

import os
import re
import sys

EXCLUDED_NAMES = {"tests.rs", "test_helpers.rs", "mock_pty.rs"}
EXCLUDED_DIRS = {"tests"}


def strip_comments_and_strings(text):
    out = []
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]
        if ch == '/' and i + 1 < n and text[i + 1] == '/':
            while i < n and text[i] != '\n':
                i += 1
            continue
        if ch == '/' and i + 1 < n and text[i + 1] == '*':
            depth = 1
            i += 2
            while i < n and depth:
                if text.startswith('/*', i):
                    depth += 1
                    i += 2
                elif text.startswith('*/', i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            out.append(' ')
            continue
        if ch in '"\'':
            quote = ch
            i += 1
            while i < n:
                if text[i] == '\\':
                    i += 2
                    continue
                if text[i] == quote:
                    i += 1
                    break
                if text[i] == '\n':
                    break
                i += 1
            out.append(' ')
            continue
        out.append(ch)
        i += 1
    return ''.join(out)


def strip_cfg_test(text):
    """移除 #[cfg(test)] 标注的项（含其函数体）。"""
    marker = '#[cfg(test)]'
    while True:
        idx = text.find(marker)
        if idx < 0:
            return text
        cursor = idx + len(marker)
        while True:
            rest = text[cursor:]
            stripped = rest.lstrip()
            cursor += len(rest) - len(stripped)
            if stripped.startswith('#['):
                end = stripped.find(']')
                if end < 0:
                    return text[:idx] + ' '
                cursor += end + 1
                continue
            break
        brace = text.find('{', cursor)
        semi = text.find(';', cursor)
        if brace < 0 or (0 <= semi < brace):
            end = semi + 1 if semi >= 0 else cursor
            text = text[:idx] + ' ' + text[end:]
            continue
        depth = 0
        i = brace
        while i < len(text):
            if text[i] == '{':
                depth += 1
            elif text[i] == '}':
                depth -= 1
                if depth == 0:
                    i += 1
                    break
            i += 1
        text = text[:idx] + ' ' + text[i:]


def production_files(root):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_DIRS]
        for name in filenames:
            if not name.endswith('.rs'):
                continue
            if name in EXCLUDED_NAMES or name.endswith('_tests.rs'):
                continue
            yield os.path.join(dirpath, name)


def scan(root):
    text = '\n'.join(
        strip_comments_and_strings(strip_cfg_test(open(p, encoding='utf-8').read()))
        for p in production_files(root)
    )
    return {
        'unwrap': len(re.findall(r'\.unwrap\(\)', text)),
        'expect': len(re.findall(r'\.expect\(', text)),
        'panic': len(re.findall(r'panic!\s*[\(\{]', text)),
        'todo': len(re.findall(r'todo!\s*[\(\{]', text)),
        'unimplemented': len(re.findall(r'unimplemented!\s*[\(\{]', text)),
        'unsafe_total': len(re.findall(r'\bunsafe\b', text)),
        'unsafe_block': len(re.findall(r'\bunsafe\s*\{', text)),
        'unsafe_fn': len(re.findall(r'\bunsafe\s+(?:extern\s+"[^"]*"\s+)?fn\b', text)),
        'unsafe_extern': len(re.findall(r'\bunsafe\s+extern\b', text)),
        'unsafe_impl': len(re.findall(r'\bunsafe\s+impl\b', text)),
    }


if __name__ == '__main__':
    print(scan(sys.argv[1]))
```

依赖活跃度采样用 crates.io 的 `https://crates.io/api/v1/crates/<name>` 与
Maven 的 `maven-metadata.xml`（Google Maven 与 Maven Central 各取最新发布元数据），
采样时刻与本留档的表格一致；重跑时 MUST 以当日实测覆盖表中数字。
