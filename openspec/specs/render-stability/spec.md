# render-stability Specification

## Purpose

渲染稳定性基线：字形首帧完整性、IME 动画期间输出完整呈现、滚动无撕裂、启动首帧及时呈现。记录设备取证结论、根因与已验证的实现细节（置信度低于 docs/specification/DESIGN.md）。

## Requirements

### Requirement: 字形首帧完整可见且幂等

任意 glyph（含合成斜体/粗体）首次渲染 MUST 完整可见，不得缺失部分字形；重复渲染结果
MUST 与首次一致（幂等）。MUST NOT 以 advance 宽度裁剪合成斜体位图的有效部分。

实现：`render_cell_data` 在实例构建完成、绘制命令提交之前补传脏区
（`take_dirty_rect` → `upload_atlas`），`write_texture` 以调用顺序入队先于本帧绘制命令
执行，保证首帧即按完整字形采样。

#### Scenario: 斜体字形首帧完整

- **WHEN** 终端首次输出合成斜体字形
- **THEN** 首帧即完整显示，无需重复执行才完整；重复渲染逐字节一致且无新脏区

### Requirement: IME 动画期间输出完整呈现

IME 弹出/隐藏动画期间产生的 PTY 新输出，在 IME 稳定后 MUST 完整呈现。渲染暂停 MUST
配对恢复；恢复后首选帧强制重绘。

#### Scenario: 输入法弹出期间输出不丢

- **WHEN** IME 弹出动画期间 PTY 产生新输出
- **THEN** IME 稳定后输出完整呈现，无缺失无错位

### Requirement: 滚动渲染无可见卡顿或撕裂

滚动渲染 MUST NOT 有可见卡顿或撕裂。

#### Scenario: 快速滚动画面连续

- **WHEN** 用户快速滚动终端内容
- **THEN** 画面连续更新，无撕裂残影（见 render-loop-scroll-cadence）

### Requirement: 启动首帧及时呈现

应用启动后终端内容 MUST 在合理时间内呈现，MUST NOT 长时间黑屏。

#### Scenario: 冷启动直接显示 Shell

- **WHEN** 应用冷启动完成
- **THEN** 启动动画结束后直接显示 Shell 与主题背景（见 DESIGN.md Shell 节）

### Requirement: 渲染 surface 失效后自愈

原生 surface MUST NOT 因其原生窗口的 BufferQueue 被遗弃而终身失效：连续 surface 级取
纹理失败 MUST 使缓存的 surface 失效，使下一次挂载走重建慢路径而非原地 reconfigure；宿主
MUST 换新的 `SurfaceView` 以取得新的原生窗口，且请求 MUST 受间隔与次数上限约束。

#### Scenario: 死窗口连续失败后置失效

- **WHEN** 连续两次取纹理返回 `Lost`/`Outdated`（`SURFACE_LOSS_STREAK_LIMIT`）
- **THEN** `surface_invalidated` 置位，MUST 只记一条 error

#### Scenario: 慢机器超时不算失效

- **WHEN** 取纹理因工作线程忙或超时而本帧跳过（`Skipped`）
- **THEN** 连续计数 MUST 清零，MUST NOT 置失效

#### Scenario: 单次 Outdated 不误判

- **WHEN** 取纹理单次返回 `Outdated`（SurfaceFlinger 缩放竞态）且下一次取到纹理
- **THEN** MUST NOT 置失效

#### Scenario: 失效后挂载走重建

- **WHEN** surface 已失效且宿主送来 `ANativeWindow`
- **THEN** `attach_surface` MUST 走重建慢路径（`attach_surface: configured`），
      MUST NOT 记 `RECONFIGURE_SWAPCHAIN`

#### Scenario: 恢复即清零

- **WHEN** 重建成功或 `detachWindow` 释放 surface
- **THEN** 失效位与连续计数 MUST 清零，新 surface 重新接受判定

#### Scenario: 失效位随打包返回上报

- **WHEN** 原生完成一帧渲染
- **THEN** 失效位 MUST 置于打包返回的第 53 位，MUST NOT 挂在渲染计数门下
      （空闲帧也会读取）

#### Scenario: 宿主换视图而非重挂同一窗口

- **WHEN** 宿主观察到失效位为 1
- **THEN** MUST 换掉整个 `SurfaceView` 以取得**新的**原生窗口；
      MUST NOT 仅对同一窗口反复 detach/attach（唤不活被遗弃的 BufferQueue）

#### Scenario: 重建请求限流

- **WHEN** 失效位持续为 1（重建无望）
- **THEN** 请求间隔 MUST ≥ 最小间隔，单会话请求次数 MUST ≤ 上限，超过后停止并告警一次

### Requirement: 帧与循环计时报告保留亚毫秒精度

帧窗口与循环窗口的汇总行 MUST 由纳秒直接换算为亚毫秒精度输出，MUST NOT 以整毫秒
截断的中间值参与换算：真机帧时长集中在 0–2ms，整除后健康状态一律报出
`avg=0ms p95=0ms`，窗口无法区分 0.1ms 与 0.9ms 两种健康基线，`frameTimingTrend`
的「退化到基线 3 倍」与 `FRAME_TIME_WARN_*` 绝对阈值告警在该分辨率下拿不到可读真值。
循环窗口的 `fps` MUST 由 `averageNanos` 直接换算；经截断均值换算的 `fps` 会把同一台
健康设备报成 55/58/62/66/71/76fps 六个值，全部是舍入产物而不含帧率信息。

#### Scenario: 健康窗口报出可读真值

- **WHEN** 真机静置一分钟，帧窗口内每帧渲染均在 2ms 以内
- **THEN** 汇总行的 `avg` 与 `p95` 为非零亚毫秒值（如 `avg=0.42ms p95=0.91ms`），
  而非 `avg=0ms p95=0ms`

#### Scenario: fps 不随舍入跳变

- **WHEN** 同一台设备循环周期稳定在 16ms 上下
- **THEN** 各窗口的 `fps` 取值一致，不出现 55/58/62/66 之间由整除产生的跳变

### Requirement: 备用屏下输入法弹出不位移终端 Surface

备用屏（helix/vim 等全屏 TUI）激活时，输入法跟随位移 MUST 为 0：此类应用恒占满
视口，任何位移都会把应用顶部推出屏幕并使视觉行与触摸换算行错位。主屏的位移公式
MUST NOT 因此改变。备用屏状态 MUST 随每帧渲染结果一并上报（与光标行、内容下沿
同批），MUST NOT 由输入法定居后的独立查询提供：输入法弹出动画期间该状态会翻转
（启动 helix 的同时键盘正收起），查询所得缓存必然滞后于当帧的位移计算。

#### Scenario: 备用屏弹出输入法不隐藏顶部

- **WHEN** helix 处于备用屏且输入法弹出，网格内容填满视口
- **THEN** 终端 Surface 位移为 0，helix 首行仍在屏幕内可见

#### Scenario: 主屏位移公式不变

- **WHEN** 终端处于主屏且内容填满网格
- **THEN** 位移仍为整块键盘高度，与备用屏判定无关

### Requirement: 着色器在测试阶段完成解析与校验

终端单元与 Kitty 图形协议的 WGSL 着色器 MUST 在 `cargo test` 阶段完成解析与校验，
MUST NOT 仅依赖设备上创建管线时才暴露错误。校验 MUST 只用 CPU，MUST NOT 依赖 GPU
适配器，校验失败 MUST 以 naga 的诊断信息作为断言失败信息输出。

着色器源码 MUST 为单一来源：管线创建与校验测试 MUST 引用同一处定义，MUST NOT 各自
内联 `include_str!`。

#### Scenario: 着色器语法错误时测试失败并给出诊断

- **WHEN** 任一着色器无法被 naga 解析或校验
- **THEN** 测试失败，输出该着色器的 naga 诊断信息

#### Scenario: 着色器合法时测试通过

- **WHEN** 两个着色器均通过解析与校验
- **THEN** 测试通过

实现细节：

- 着色器源码为 `render/pipeline.rs` 的 `CELL_SHADER` 与 `KGP_SHADER` 两个常量，
  由 `include_str!` 编译期内联；管线创建与校验测试共用，不存在第二处内联。
- 校验经 `wgpu::naga`（`wgpu` 无条件再导出 `wgc::naga`）完成，无需新增依赖，也无需
  GPU 适配器，因此在无 lavapipe 的环境下同样执行而非跳过。
- 校验分两步且分别断言：先 `wgsl::parse_str` 解析，再以
  `Validator::new(ValidationFlags::all(), Capabilities::default())` 校验 IR；两步失败
  均以 `emit_to_string` 的 naga 诊断作为断言信息。
