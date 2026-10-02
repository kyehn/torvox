# 修复宿主链接失败：rust-lld 拒绝 Zig compiler_rt 的 null 符号引用

## Why

`check` 与 `build` 两条工作流（run / ）均在链接阶段失败：

```
rust-lld: error: undefined symbol:
>>> referenced by compiler_rt
>>> compiler_rt.o:(.rodata.compiler_rt.sincos.sincosf+0x0) in archive .../liblibghostty_vt_sys-*.rlib
collect2: error: ld returned 1 exit status
```

这不是 CI 偶发：`cargo clean --package libghostty-vt-sys` 后本地全新构建**同样**
失败，说明既有绿灯都来自陈旧的 ghostty 构建产物。

根因链（本地取证）：

1. `libghostty-vt.a` 内含 Zig 0.16.0 的 `compiler_rt.o`。其中
   `compiler_rt.ssp.__memmove_chk` 的重定位为
   `R_X86_64_PLT32 → 符号表第 0 项（NOTYPE LOCAL UND，名字为空）`：Zig 把 null
   符号当作不可达陷阱的落点，**不要求链接期解析**。
2. rustc 自 1.90 起在 `x86_64-unknown-linux-gnu` 默认改用 rust-lld；lld 对 null
   符号引用直接判未定义并终止链接 → 主机 cdylib、单测二进制与 bench 全部链接失败。
   同一归档用 GNU ld 链接成功（null→0 即该引用的本义）。
3. Zig 把机器码相同的 `__*_chk` 包装合并为同一函数，`.eh_frame` 因此重叠，
   ld.bfd 的 `--eh-frame-hdr` 排序阶段拒绝（`.eh_frame_hdr refers to overlapping
   FDEs` → `final link failed: bad value`）。

## What Changes

- 新增 `.cargo/config.toml`，仅对 `x86_64-unknown-linux-gnu` 追加两个链接参数：
  `-C link-arg=-fuse-ld=bfd`（用按 null→0 本义解析的 GNU ld）与
  `-C link-arg=-Wl,--no-eh-frame-hdr`（跳过重叠 FDE 排序；该头只是 unwind 快速
  索引表，`.eh_frame` 与 libunwind 行为不变）。
- 不使用任何「忽略未定义符号」类参数：错误照旧报出，只是换一个能正确实现
  该对象文件所请求语义的链接器。

## Non-goals

- 不改 `flake.nix` / `rust-toolchain.toml`（保护文件）：Zig 版本与工具链口径不动。
- 不改 `libghostty-vt-sys`（外部依赖）与 `Cargo.toml`。
- 不影响 Android 目标：`cargo-ndk` 自带 lld，且 Android 链接不经 Zig 归档。