任何情况下禁止对本文件进行任何修改

---

## 未被解决的问题

基于11a236bd3ddc34241fa7460fedd2be903b4b4653提交

- （已解决 2026-11-19）软件启动从出现背景到shell需要较久时间，概率性发生
  - 根因：平台偶发的 Vulkan 设备丢失（Mali-G57，真机日志 2026-11-19 17:44 复现）后，
    宿主随每次 `surfaceCreated`/`resumeRendering` 把换视图预算复位，快试 5 次上限
    形同虚设；实测 2.8s 内换掉 15 个 SurfaceView、每 ~50ms 重建一次 VkInstance，
    首屏可见但 shell 提示符画面约延迟 2s 才出现。
  - 修复：换视图预算只在回到前台（`ON_RESUME` / `resumeRenderingFromBackground`）
    时清零；同一前台内设备丢失不再因每次 attach 复位，快试 5 次（间隔 ≥ 500ms）后
    转为慢重试（每 5s 一次），渲染成功后由 RenderSupervisor 复位。
