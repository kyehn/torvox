# 证据留档：窗口收回后的日志分级

按 `logcat-hygiene` 规范「外部与上游条目必须逐条归因并留档」，本文件只记录聚合结论、
分级对照与复现步骤；原始日志行、时间戳、PID、设备标识与下载链接不入库。

## 复现步骤

1. 在软件 Vulkan 的模拟器上安装 debug 构建并启动终端。
2. 按 Home 切到后台——平台此时收回原生窗口（BufferQueue 被遗弃）。
3. 切回前台，等待渲染线程完成换窗口与首帧呈现。
4. 抓取应用进程内的日志，按级别与来源统计。

## 分级对照（同一复现路径，修复前后）

| 条目 | 修复前 | 修复后 | 归因 |
| --- | --- | --- | --- |
| wgpu 重新配置 surface 失败 | error | debug | 平台收走窗口的连带结果 |
| 渲染帧取不到纹理 | error | debug | 同上 |
| 宿主侧瞬时渲染错误汇总（首次） | warn | info | 同上 |
| 宿主侧瞬时渲染错误汇总（连续出现） | warn | warn | 新窗口迟迟不来，属真异常 |

## 复现结果（一次后台往返的条目计数）

应用自身：

- error：0
- warn：9——全部为慢帧／慢循环诊断（规范允许，但要求 500ms 窗口限频）；
  本次往返共 9 条，未出现「帧失败」「瞬时渲染错误」「surface 失效」类条目
- info：1——瞬时渲染错误汇总首次出现
- debug：2——wgpu 重新配置 surface 失败、渲染帧取不到纹理

平台与上游（无应用栈，不绕过）：

- 平台 BufferQueue 被放弃：2
- 平台 Vulkan 取缓冲失败：2
- 上游 wgpu-hal 上报 surface 丢失：1

恢复证据：一次失败帧后即恢复——宿主记录「recovered after 1 errors」，下一帧成功呈现，
窗口重挂载后配置与呈现均恢复。

## 自动化判据

- Rust：`gpu_error_recoverability`（可恢复／不可恢复）、`gpu_error_classification`
  （上游文案命中、不命中、非校验错误不命中）四个用例。
- Kotlin：`TransientRenderErrorSeverityTest`（首次非 warn、连续仍 warn）两个用例。
- Rust 全量 `cargo test --lib` 与 Kotlin 全量 `:app:testDebugUnitTest` 全绿。
