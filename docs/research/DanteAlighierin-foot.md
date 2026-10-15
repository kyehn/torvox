# DanteAlighierin/foot 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/foot`
- 技术栈：C + Wayland (xdg-shell, fractional scale), self-developed VT parser
- 定位：极简 Wayland terminal (foot), fast CPU rendering with glyphs
- 许可证：MIT
- 主要模块：csi/dcs/osc/extraction/rendering, box-drawing, composed Unicode, cursor shapes, config.

## 全仓库结构与关键代码
- `csi.c/h`, `dcs.c/h`, `commands.c/h` self-dev VT.
- `box-drawing`, `composed`, `char32`: Unicode handling.
- Wayland event loop, client protocol, fractional scaling.
- No GPU? foot uses CPU (wl_shm) rasterization usually, shaper.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- foot Wayland-only, CPU render, self-dev VT; not Android.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- CSI/DCS parser structure.
- box-drawing synthesis, composed emoji width.
- Wayland fractional scaling handling (desktop).

## 可稍微修改使用的代码/设计
- C parser not adopt; VT single-source policy.

## 测试数据/测试用例
- Tests include compile-time/VT test files; exact runnable harness was not catalogued.

## 设计模式
- C terminal modular.
- Self-contained config.

## 其他结论
- C-grade desktop (Wayland). Not direct.
- MIT friendly.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
- Repository branding is reported as foot (dnkl/foot upstream) in its metadata; this report does not verify fork/mirror relationship with upstream.
