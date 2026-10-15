# contour-terminal/contour 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/contour`
- 技术栈：C++23 + CMake/VCPKG + Qt?、OpenGL GPU 渲染、自研 VT parser/vtbackend
- 定位：现代快速终端模拟器，提供持久会话、Vi 模式、图像协议、GPU 渲染
- 许可证：Contour Terminal_license? Apache 2.0? (LICENSE.txt)
- 主要模块：`vtparser`, `vtbackend`, `vtrasterizer`, `contour` UI, tests 377+，conformance/vt_workspace

## 全仓库结构与关键代码
- `src/vtparser`：VT parser + bulk scan + UTF8 解码 + events + extension; `Parser_test.cpp`.
- `src/vtbackend`：core/grid/screen/input/graphics/rendering/shell/testing/vt。
- `src/vtrasterizer`：OpenGL 字形图集、背景、装饰、图片、文本渲染、测试。
- `src/vtconformance`：VT consistency/conformance tests with expected outputs.
- `src/vtworkspace`：headless workspace and bench.
- daemon/client, persistent sessions, session protocol.

## 对比本项目
- 本项目 Android Rust + wgpu/Vulkan + Compose + Ghostty VT.
- Contour 自研 VT backend，GPU rendering 使用 OpenGL，与 Ghostty 单一来源/Android Vulkan 不兼容.
- Contour 的 conformance suite and rasterization code high-value reference.
- Persistent sessions, Vi selection, image protocol, glyph rasterization algorithms can inform.

## 可直接使用的库/工具
- 无 Android/Kotlin 依赖；C++ not directly embed.
- vcpkg ecosystem dependencies not fit Android.

## 可间接借鉴
- VT parser event design and bulk scan (only if self-developing, not applicable).
- Conformance tests as external truth candidates (expected output must be audit).
- Glyph atlas, rasterization, box drawing, underline geometry, text cluster grouping, image texture.
- synchronized output, reflow, OSC8, sixel/regis.
- daemon/persistent session protocol.

## 可稍微修改使用的代码/设计
- C++ code not portable to Rust directly, but algorithms and shaders inform.
- Conformance tests need host/expected data conversion and self-validation audit.
- license and STYLE constraints.

## 测试数据/测试用例
- Repository is described with roughly 377 tests; this report did not enumerate test modules by responsibility.
- `src/vtconformance` offers systematic VT test data and expectations.
- Conformance expected outputs can help本仓 integration tests if passes upstream-alignment 3 criteria.

## 设计模式
- modular vtbackend + parser + rasterizer
- testing mode for headless
- conformance test suite
- trailing: daemon/client

## 其他结论
- C++/OpenGL desktop; not direct. Conformance/rasterization high-value.
- Self-developed VT conflicts with single-source principle; only reference.

## 四次审阅记录
- 准确性：基于 source dirs.
- 完整性：涵盖 architecture and comparison.
- 对比正确性：stresses VT single-source conflict.
- 文档一致性：仅 md.
