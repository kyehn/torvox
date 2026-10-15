# harmoninux/HiSH 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/hish`
- 技术栈：HarmonyOS (ArkTS/Hvigor/C++) + QEMU (`libqemu-system-aarch64`) 运行完整 ARM64 Linux 内核
- 定位：HarmonyOS 设备上的 Linux 虚拟机 (2in1/tablet/phone), 类似 Termux but full distro VM
- 许可证：HiSH 需注明项目/仓库 (custom)
- 构建：DevEco Studio，hap/hpk; 可选 native qemu libs and kernel/rootfs
- 无传统终端 VT parser in_repo; 通过 guest terminal (Alpine/Ubuntu) 体现.

## 全仓库结构与关键代码
- `feature/hish_main` HarmonyOS app entry.
- `product/phone`, `product/tablet` product configs.
- `deps`：zstd/pcre2/pixman/zlib/glib/libqemu.
- `docs/guide`: usage (portrait, landscape, port forwarding, theming).
- 无 Compose/WGPU terminal code; essentially VM orchestrator.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose terminal, Ghostty VT.
- HiSH runs Linux under QEMU, not a terminal emulator; 不是 VT 实现 nor renderer.
- HarmonyOS ecosystem differs from Android; no TorVox competitor or runtime target was derived here.
- Android 无法直接用 HarmonyOS product, QEMU requires Android host complexities.

## 可直接使用的库/工具
- 无.
- QEMU 可作为 Android host? Possible but heavy, not suitable; licensing conditions.

## 可间接借鉴
- 虚拟机 lifecycle、image download、storage, port forwarding.
- Keyboard special keys approach for mobile.
- Product packaging and sharing options.

## 可稍微修改使用的代码/设计
- HarmonyOS app code cannot port to Android.
- qemu-based RISC-V Android execution paths were not tested; runtime assumptions remain out of scope for this report.

## 测试数据/测试用例
- 无 terminal VT tests.

## 设计模式
- app VM orchestrator (HarmonyOS).
- optional native libs/kernels.

## 其他结论
- 不适合作为 Android 终端 VT/render reference.
- 对 Torvox 产品定位来说，HiSH 是 full-VM mobile Linux rather than terminal emulator; not direct.
- Terms require attribution.

## 四次审阅记录
- 准确性：README, dirs.
- 完整性.
- 对比正确性.
- 文档一致性.
