# 运行日志审计（第四轮）

聚合口径：两份真机运行日志（常规使用 + `kitten icat`/`clear` 复现），只统计
W/E/F 级；按 `openspec/specs/logcat-hygiene/spec.md` 的要求，本文不记录原始日志行、
时间戳、PID、设备标识与下载链接。

## 归因总表

| 条目 | 级别 | 条数 | 归属 | 结论 |
| --- | --- | --- | --- | --- |
| `mali_gralloc` 格式探测（R8/未识别格式） | E | 72 | 平台/驱动 | 驱动在 `vkCreateSwapchainKHR` 内逐 image 探测后回退；应用已固定 RGBA_8888，无可干预点（详见 `docs/STATUS.md`） |
| `SurfaceSyncer: Failed to find sync for id=N` | E | 30 | 平台 | 只出现在 surface 建立/拆除路径；id 由框架事务分配，应用无写入点 |
| `Parcel: Expecting binder but got null!` | W | 15 | 平台 | 与上一项成对出现，框架打包系统窗口事务时的诊断 |
| `InputEventReceiver: ... receiver has already been disposed` | W | 4 | 平台 | 视图/输入通道在事件处理中被拆除时的框架告警；与本轮记录的「surface 反复重建」同源，属症状而非独立缺陷 |
| `libc: Access denied finding property "<vendor 属性>"` | W | 8 | 平台/ROM | 厂商标识与调试属性，应用无权读取也不应读取 |
| `ziparchive`、`Typeface`、`FeatureOpt` | W/E | 14 | 平台/ROM | 系统镜像与机型定制日志（缺失 framework 描述文件、默认字体更新、折叠屏特性探测） |
| `com.termux: Not starting debugger ...` | E | 2 | 平台 | debuggable 构建的系统 JDWP 尝试，release 不出现 |
| SELinux `avc: granted/denied`（tag 为 fish/eza/yazi/login/bash/clear/comma/DefaultDispatch/chroot-ng） | W | 60 | 内核审计 | `auditd` 按进程名转发的内核审计记录，不是这些程序写出的日志；`niceness` 与策略由 ROM 决定 |
| `kitty_gfx`：`failed to open image file` → `EINVAL: invalid data` → `EINVAL: unsupported medium` | W | 4 | 上游（libghostty-vt） | 客户端按「共享内存 → 文件 → 直接传输」顺序探测介质：`t=s` 平台不支持、`t=f` 路径在应用进程内不可达，最终由直接传输成功出图。协议按规范给出 `ERROR:` 应答，客户端据此回退；应用不伪造未定义应答 |
| `stream: unimplemented CSI action: ESC [ * x` | W | 1 | 上游（libghostty-vt） | 上游未实现的 CSI 动作；应用不绕过上游、不改写协议语义 |
| 应用自身诊断：`SLOW_FRAME`/`SLOW_LOOP`/`SLOW_RENDER_INNER`/`SLOW_CELL_BUILD`/`SLOW_RENDER_SAMPLE` | W | 16 | 应用自身 | 均已按 500ms 窗口限频，且各段时长自洽：全部命中在首帧建管线、首次全屏单元构建与一次 30ms 边界采样，属可解释的冷路径，不是稳态退化 |

## 处置

- 应用自身条目：本轮无新增缺陷；限频与分段口径见 `logcat-hygiene` 规范。
- 上游条目：记录事实，不绕过、不伪造应答。`t=f` 路径不可达的根因是「客户端给出的路径
  相对应用进程的 cwd」，改变进程 cwd 会污染其余相对路径调用，故不做。
- 平台/驱动条目：见 `docs/STATUS.md` 的「已判定为非应用缺陷」小节，逐条附判据。
- `InputEventReceiver`：并入 `docs/STATUS.md` 的「切换应用返回页面闪烁」一项跟踪
  （两处同源于 surface 反复重建）。

## 未纳入统计

- Debug/Info 级条目（渲染帧摘要、字体族解析、会话生命周期）只用于排障，不参与 W/E 审计。
- 应用自身的 `native::*` Debug 行在 release 构建中不输出。
