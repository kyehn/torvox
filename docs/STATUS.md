任何情况下禁止对本文件进行任何修改

---

## 未被解决的问题

基于11a236bd3ddc34241fa7460fedd2be903b4b4653提交

- （有界缓解 2026-11-19，非彻底修复）软件启动从出现背景到shell需要较久时间，概率性发生
  - 根因（已定位、未消除）：平台偶发的 Vulkan 设备丢失（Mali-G57，真机日志
    2026-11-19 17:44 复现）后，宿主随每次 `surfaceCreated`/`resumeRendering` 把换视图预算
    复位，快试 5 次上限形同虚设；实测 2.8s 内换掉 15 个 SurfaceView、每 ~50ms 重建一次
    VkInstance，首屏可见但 shell 提示符画面约延迟 2s 才出现。设备丢失本身是 Mali/SPRD
    驱动侧事件，应用侧无证据可消除（两簇发生后应用继续正常渲染）；此条目不作“已解决”表述。
  - 已落地（应用侧）：换视图预算只在回到前台（`ON_RESUME` / `resumeRenderingFromBackground`）
    时清零；同一前台内设备丢失不再因每次 attach 复位，快试 5 次（间隔 ≥ 500ms）后转为
    慢重试（每 5s 一次），渲染成功后由 RenderSupervisor 复位；设备丢失余波（panic 副本、
    cell metrics、render skipped）不再各记一条 error。
  - 未解决（平台/根因）：
    - Mali-G57 首帧后约 6ms 设备丢失的确切成因未能定位（本机无真实 Mali：mobile-mcp 列出仅 `test_avd` 模拟器，不能复现）；应用侧缓解只限制重建次数，不能避免丢失本身。
    - `SLOW_CELL_BUILD`（997 单元 / 575 实例，build≈22ms）出现在滚动输出后的首屏重输出帧；本机热路径 `cell_builder` bench ≈0.358ms、端到端预热路径 3.0ms/帧，无算法回归，仍需真机 profiler 再决定是否优化。
    - `mali_gralloc` R8 探测、`wgpu_core::instance` downlevel 提示、`Parcel` binder null、
      `libc` 属性、`ziparchive`、`FeatureOpt`、ZTE `Typeface`、debug jdwp 等均为平台/厂商/
      debug 条目，归因为平台，未降级、未过滤、未改阈值。
