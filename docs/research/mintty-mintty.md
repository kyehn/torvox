# mintty/mintty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/mintty`
- 技术栈：C/Win32 + Cygwin/MSYS POSIX 层，Makefile，自研终端 + 字符集/Unicode 宽度/组合表生成器
- 定位：Cygwin/MSYS 终端模拟器 + 图像协议 (Sixel/Regis/Tektronix)
- 许可证：GPL (LICENSE.GPL)；包含 PuTTY/Oxygen 等第三方授权
- 代码规模：src ~249 文件，大量生成的数据表

## 全仓库结构与关键代码
- `src/term*.c`：终端状态、行缓冲、输出、鼠标、剪贴板、扫描。
- `src/charset.c`、`*.t` 表：Unicode/EAW/组合字符/全角/bidirectional 等数据生成。
- `src/minibidi.c/h`：Bidi 算法。
- `src/sixel.c/h`、`regis.c/h`、`tek.c/h`、`winimg.c/h`：图像协议。
- `src/mcwidth.c`：细胞宽度计算。
- `src/win*.c`：Windows 控制、输入、绘制、tab、剪贴板、搜索。
- `docs/`、`wiki/`：文档。

## 对比本项目
- 本项目 Android Rust + wgpu/Vulkan + Kotlin/Compose + Ghostty VT。
- Mintty 是 Win32/Cygwin C 项目，平台与 VT 实现均异构。
- 作为自研终端，与本仓 Ghostty 单一来源冲突；不可 adopt VT 核。
- 但其 Unicode 宽度、 ambiguous width、EAW、combining tables 与 Bidi 实现可作算法/数据参考。

## 可直接使用的库/工具
- 无 Rust/Kotlin/Android 依赖。
- Makefile/生成器工具可参考自动生成 Unicode 表，但本仓 license 限制（GPL）与无 vendor 政策。

## 可间接借鉴
- mcwidth / 组合字符/全角/Emoji 宽度测试逻辑。
- ambiguous-width 处理与 CJK 字体。
- Sixel/Regis 图像协议的坐标与配色处理（未列入当前范围）。
- minibidi 实现 (本仓规范当前不含 Bidi，可留作参考)。
- 终端搜索、剪贴板、滚动、鼠标选择的 Win32 交互方式（非 Android）。

## 可稍微修改使用的代码/设计
- C Unicode 表生成代码不能直接移植；但可重新生成 Kotlin/Rust 数据；license 需重新确认。
- Sixel/Regis are protocol implementation references; TorVox does not declare its own image protocol. Ghostty's VT supports Kitty graphics, and no Sixel display behavior is part of this project's stated scope.
- Bidi 实现仅作对照，本仓当前不支持 Bidi。

## 测试数据/测试用例
- `testbidi.tc` 与生成脚本；用例以文本表。
- 无 Android/instrumentation 测试；更多是数据生成。

## 设计模式
- C 结构体 + 全局状态。
- 编译期 Unicode tables。
- 事件主循环 Win32。

## 其他结论
- mintty is GPL-2.0-only; direct source reuse requires GPL compliance, which TorVox's STYLE prohibits.
- 对本仓 primarily algorithm/protocol reference, not dependency.

## 四次审阅记录
- 准确性：基于 src/docs.
- 完整性：涵盖 repo, libraries, indirect, modifications, tests, patterns.
- 对比正确性：明确 C/Win32 与 Android/Rust/Compose incompatibility.
- 文档一致性：仅 md.
