# 任务

## 一、采集通道

- [x] `FrameStats`：解析 `dumpsys gfxinfo` 的帧数、janky 占比与 p50/p90/p95/p99。
- [x] `printMeasured`：实测值 `println` 进 instrumentation 输出流并 `Log.i("Benchmark")`
      进 logcat（两条通道各留一份）；连接测试任务按用例把设备 logcat 落进构建产物
      （`androidTest-results/connected/<variant>/<device>/logcat-*.txt`），实测值因此可复核。
- [x] 交互三用例与启动两用例逐轮打印实测耗时与帧统计。
- [x] `LogcatRecord` + `readLogcatSince`：设备侧采集按 `Runtime.exec(String)` 的
      真实语义构造命令（不经 shell：按空白切词、引号原样传参、管道不生效），
      logcat 过滤词为不含空白的 POSIX ERE，本地再按记录解析复核。

## 二、滚动宏基准

- [x] `ScrollPerformanceBenchmark`：`input text` 敲入 `seq 500` 填满回滚区，
      上滑、下滑各一次（固定 1.5s 拖动时长，避开底部修饰键栏）。
- [x] shell 就绪判定：shell 无提示符可等，用幂等探针命令（`echo`）诱发首个 PTY
      输出作为就绪信号（探针不回显即判红，不拿固定等待时长顶替）。
- [x] 帧级数据面改为原生渲染器的呈现汇总（`render_frame: presented N frames`）：
      终端内容直绘 SurfaceView，gfxinfo（HWUI）记不到这些帧。
- [x] 断言：呈现帧数 ≥ 下限、有帧的呈现窗口数 ≥ 下界、总耗时 ≤ 预算；实测值先于
      断言输出（`emitMeasured`），门禁失败时构建产物里仍留得下这轮数字。

## 三、按实测校准断言

- [x] 本机 AVD（API 35 / SwiftShader）实测：空闲宿主三轮滚动耗时 3550/3564/3585ms、
      呈现帧数 209/209/212 帧、有帧窗口 8 个；宿主被并发任务抢占时同一构建只呈现
      65 帧、窗口仍为 8 个。耗时预算按约 1.5 倍口径定为 6s；帧数下限按**最差实测**
      的一半定为 30；另加「有帧窗口数 ≥ 4」这一与宿主速度无关的判据。
- [x] 断言阈值改为「最差实测」口径：呈现帧数是宿主吞吐的函数，按空闲宿主的峰值
      设限会把满载宿主上的健康构建判成回归（实测 65 < 100 即误判）。
- [x] 六个用例逐轮实测（本机空闲宿主）：冷启动 2097-2234ms/23-25 帧，热启动
      1074-1149ms/3-4 帧，修饰键 42-61ms/1 帧，会话抽屉 680-1197ms/6 帧，
      IME 弹出 1053-1068ms/21 帧，滚动 3550-3559ms/209-213 呈现帧。
- [x] 启动两用例补上帧数断言（此前只断言耗时与界面出现，不满足「每个用例至少
      断言一帧」），并把实测值输出移到断言之前，断言失败时仍留得下这轮数字。
- [ ] CI 恢复运行后用 CI 上的 `[bench-measure]` 实测值复核同一组阈值（CI 模拟器
      只给 2 核，比本机更慢，届时按实测重新校准）。
- [ ] 启动两用例的 6s/3s 预算按实测中位数收紧到同口径。

## 四、门禁

- [x] `openspec validate --all --strict` 通过
- [x] `:benchmark:connectedBenchmarkReleaseAndroidTest` 本机全绿（6/6）
- [x] `detekt` + `spotlessCheck` 通过
- [ ] CI check + build 全绿（提交后由 CI 确认；当前 CI 由仓库所有者暂停中）
