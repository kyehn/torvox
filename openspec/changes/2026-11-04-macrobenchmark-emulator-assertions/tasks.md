# 任务

## 一、采集通道

- [x] `FrameStats`：解析 `dumpsys gfxinfo` 的帧数、janky 占比与 p50/p90/p95/p99。
- [x] `printMeasured`：实测值 `println` 进 instrumentation 输出流（测试 XML
      system-out）并 `Log.i("Benchmark")` 进 logcat。
- [x] 交互三用例与启动两用例逐轮打印实测耗时与帧统计。
- [x] `scripts/test-emulator.nu`：宏基准跑完 cat 测试 XML，实测值进构建日志。

## 二、滚动宏基准

- [x] `ScrollPerformanceBenchmark`：`input text` 敲入 `seq 500` 填满回滚区
      （帧计数稳定即渲染完毕：光标不闪烁，静止终端不出帧），上滑、下滑各一次。
- [x] 断言：至少出帧、总耗时在预算内；帧统计随 `printMeasured` 落进 CI。

## 三、按实测校准断言（依赖 CI 恢复运行）

- [ ] CI 构建日志出现 `[torvox-bench]` 实测行后，取各用例实测 p50/p90 与耗时
      的约 1.5 倍设为断言预算（与 Rust 微基准上限断言同口径），注释引用 CI 实测值。
- [ ] 启动两用例的 6s/3s 预算按实测中位数收紧到同口径。

## 四、门禁

- [x] `openspec validate --all --strict` 通过
- [ ] CI check + build 全绿（提交后由 CI 确认；当前 CI 由仓库所有者暂停中）
