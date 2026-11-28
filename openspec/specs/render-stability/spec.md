# render-stability Specification

## Purpose

渲染稳定性基线：字形首帧完整性、IME 动画期间输出完整呈现、滚动无撕裂、启动首帧及时呈现。记录设备取证结论、根因与已验证的实现细节（置信度低于 docs/specification/DESIGN.md）。

## Requirements

### Requirement: 系统栏图标明暗跟随所在页面的底色

窗口是边到边的，系统栏自身透明，只决定**图标明暗**。每一页 MUST 按**该页自己的底色**
亮度决定系统栏图标明暗：终端页按已解析的终端主题背景，设置页按设置页的表面色。
MUST NOT 按系统深浅色或软件主题开关决定——那些与应用内的浅色页面无关，会让浅底上的
白字图标不可读。

#### Scenario: 浅色设置页用深色图标

- **WHEN** 打开设置页（浅色表面）
- **THEN** 状态栏与导航栏图标取深色，压在浅色底上清晰可读

#### Scenario: 深色终端页用浅色图标

- **WHEN** 停留在终端页（终端主题背景为深色）
- **THEN** 状态栏与导航栏图标取浅色，压在深色底上清晰可读

#### Scenario: 被遮住的页面不得写系统栏

- **WHEN** 设置页覆盖在终端页之上，而终端页因新输出发生重组
- **THEN** 被遮住的终端页 MUST NOT 改写系统栏图标明暗——系统栏是窗口级单值，
  最后一个写入者获胜；两个页面同时在组合树里，各自的无条件写入会让图标明暗
  取决于重组顺序

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
配对恢复；恢复后首选帧强制重绘。暂停期间 MUST NOT 消费单元帧：被取走又丢弃的帧
不会补发，帧必须留在交付点等待恢复后呈现。

#### Scenario: 输入法弹出期间输出不丢

- **WHEN** IME 弹出动画期间 PTY 产生新输出
- **THEN** IME 稳定后输出完整呈现，无缺失无错位

#### Scenario: 输入法弹出期间连续输出

- **WHEN** IME 弹出动画期间 PTY 连续产生多批新输出
- **THEN** IME 稳定后呈现的是最后一批输出，无缺失无错位

### Requirement: SGR 隐藏文字不生成前景

SGR 8（conceal）MUST 从 VT 样式透传到生产 `CellData`，并 MUST 在实例构建时跳过
字形、下划线、删除线与上划线；背景、搜索高亮、选区和光标 MUST 保持可见。行为与
xterm 一致，MUST NOT 只更新仅供测试的快照。

#### Scenario: 隐藏文字仍保留单元格背景

- **WHEN** 程序输出带自定义背景的 `ESC[8m` 文本
- **THEN** 该单元格不生成字形或文字装饰实例，背景四边形仍正常生成

### Requirement: 滚动渲染无可见卡顿或撕裂

滚动渲染 MUST NOT 有可见卡顿或撕裂。

#### Scenario: 快速滚动画面连续

- **WHEN** 用户快速滚动终端内容
- **THEN** 画面连续更新，无撕裂残影（见 render-loop-scroll-cadence）

### Requirement: 渲染状态锁只在锁外确有工作时放开

渲染线程持渲染状态锁推进一帧时，MUST NOT 在锁外没有工作时放开这把锁再取回：
放开只给旁路线程（UI 线程的滚动回调、主题与字体变更）开一个抢锁窗口，渲染线程
在窗口里被挤到队尾，一帧可空等数十毫秒（真机实测 39 毫秒）。

锁外工作只有两项：把单元格像素尺寸同步到终端侧（须按全局锁序先放开渲染状态锁，
否则构成反向嵌套），与重查 kitty 放置（不能在锁内等 VT 回复）。两项都不做时
（纯文本稳态）MUST 保持锁不放手。

#### Scenario: 纯文本稳态不放锁

- **WHEN** 一帧既不同步单元格尺寸也不重查 kitty 放置
- **THEN** 渲染线程 MUST 自始至终持有渲染状态锁，MUST NOT 放开后再取回

#### Scenario: 锁外工作仍按锁序进行

- **WHEN** 一帧需要同步单元格尺寸或重查 kitty 放置
- **THEN** 渲染线程 MUST 先放开渲染状态锁再做该工作，取回后再继续绘制

### Requirement: 同步输出期间的帧发布门控

模式 2026（BSU/ESU）保持期间 MUST NOT 向渲染线程发布中间帧。

#### Scenario: BSU..ESU 之间不出现中间帧

- **WHEN** 程序写入 `ESC[?2026h` 后继续输出、再写入 `ESC[?2026l`
- **THEN** 保持期间不发布 CellData 帧，屏幕停留在此前的完整帧
- **AND** ESU 到达后发布包含全部保持期间输出的最终帧

#### Scenario: 保持超时后强制发布

- **WHEN** 程序发出 BSU 后超过 1s 未发 ESU（崩溃或忘记释放）
- **THEN** 宿主 MUST 复位模式并发布当前帧，MUST NOT 永久冻结画面

### Requirement: 启动首帧及时呈现

应用启动后终端内容 MUST 在合理时间内呈现，MUST NOT 长时间黑屏。

#### Scenario: 冷启动直接显示 Shell

- **WHEN** 应用冷启动完成
- **THEN** 启动动画结束后直接显示 Shell 与主题背景（见 DESIGN.md Shell 节）

#### Scenario: 启动屏遮挡到终端首帧呈现

- **WHEN** 冷启动后系统启动屏求值撤下时机（内容视图首次预绘制）
- **THEN** 终端尚未向 Surface 呈现任何一帧时 MUST 继续遮挡（此时 SurfaceView 的开孔区透出黑底）
- **AND** 终端首帧呈现后 MUST 立即撤下
- **AND** 首帧在 5s 上限内始终未呈现时 MUST 撤下，MUST NOT 永久遮挡界面

### Requirement: 切后台返回时开孔必须被覆盖且重新出帧

`SurfaceView` 的 surface 在 `onStop` 之后被系统拆除时，宿主 MUST 立即回到「无缓冲区」
状态并遮住开孔，直到新 surface 提交首帧。该状态 MUST NOT 依赖随生命周期停止收集的
订阅方式：写入发生在 `onStop` 之后，`ON_START` 重启收集与返回后的首帧绘制会抢跑。

遮罩 MUST 画在 `SurfaceView` **自己**的 `onDraw` 里，MUST NOT 画在组合层（窗口层）：
平台在 `SurfaceView` 绘制时先给应用窗口打洞，洞里窗口层画过的像素一律被抹掉。

遮罩 MUST 用**离开时的画面**（而非仅一块平底色），该画面 MUST 在设备仍存活时缓存
（见下场景）。发布遮罩 MUST 直接触发 `SurfaceView` 重绘，且这次重绘 MUST 落在平台拆
surface 之前。

返回前台后 MUST 重新出帧，MUST NOT 停留在渲染暂停状态形成永久黑屏。

#### Scenario: 切后台拆除 surface

- **WHEN** 应用切到后台且系统拆除了 `SurfaceView` 的 surface
- **THEN** 开孔覆盖状态 MUST 落回「无缓冲区」，由 `SurfaceView` 在 `onDraw` 里遮住

#### Scenario: 返回前台首帧前不透出上一个应用

- **WHEN** 应用回到前台，新 surface 尚未提交首帧
- **THEN** 开孔 MUST 显示终端自己的画面（离开时的画面或终端底色），MUST NOT 透出
      桌面或上一个应用
- **AND** 首帧按「有缓冲区」绘制时开孔区会透出桌面或上一个应用；平底色与桌面都是
      「闪一下」，只是闪成另一种颜色

#### Scenario: 返回前台后重新出帧

- **WHEN** 应用回到前台且新 surface 已挂载
- **THEN** 终端 MUST 在 30s 内重新提交并呈现帧，覆盖状态随首帧撤下

#### Scenario: 遮罩画在 SurfaceView 自己的绘制里

- **WHEN** 宿主把开孔覆盖画在组合层（窗口层）
- **THEN** 覆盖 MUST NOT 出现在开孔里——平台给窗口打洞时会把该处像素抹掉（实测：
      组合层涂的底色只在 `SurfaceView` 之外的边距上可见）
- **AND** 覆盖 MUST 画在 `SurfaceView` 的 `onDraw` 里（打洞在其之前，`onDraw` 在其之后）

#### Scenario: 窗口 surface 被拆导致交换链重建

- **WHEN** 平台在切后台后拆掉本应用**窗口**的 surface
- **THEN** 返回前台时交换链 MUST 重建，开孔 MUST 由遮罩覆盖重建期间的空档

#### Scenario: 运行期缓存离开时的画面作为开孔遮罩

- **WHEN** 终端向 surface 呈现了一帧且设备仍存活
- **THEN** 宿主 MUST 在渲染线程之外回读该帧并缓存为开孔遮罩：首帧立即回读，
      其后至多每 `COVER_CAPTURE_INTERVAL_MS`（600ms）追赶一次，MUST NOT 把回读
      变成逐帧的 GPU 同步等待
- **AND** 回读 MUST NOT 在渲染线程与主线程上完成（它是一次 GPU 同步等待）
- **AND** 回读 MUST NOT 在持有全局渲染锁时进行：锁内等待会把**所有**会话的渲染
      线程一起卡住（实测另一会话的渲染采样多花 38.98ms，触发 SLOW_FRAME 告警）。
      故锁内只取句柄快照，GPU 等待在锁外做
- **AND** 切后台的路径 MUST **先**发布已缓存的帧，再强制回读一次：周期回读被节流，
      缓存可能落后于最后一帧（实测应用刚起、缓存还是 warmup 的空白帧时切后台，
      遮罩是一块空白，与没有遮罩无异）。强制回读 MUST 绕过节流与暂停检查（它只读
      累加器纹理、不碰窗口表面），回读到的新帧 MUST 替下已发布的旧帧；回读失败
      （设备已在后台丢失）时 MUST 保留已发布的缓存，MUST NOT 退回平底色
- **AND** surface 销毁路径 MUST 只发布缓存、不做强制回读：此时设备往往已随 surface
      一同丢失，再回读只会多一条失败日志（切后台那次已在设备存活时刷过）
- **AND** 回读失败时 MUST 保留上一份缓存（旧一帧仍比平底色接近离开时的画面）
- **AND** 遮罩与当前开孔的尺寸不一致时 MUST 按宽度等比缩放、顶边对齐地贴合
      （等比缩放不拉伸字形），MUST NOT 因尺寸不一致整块放弃：输入法弹出/收起
      会改开孔高度，整块放弃就退回平底色——即「输入法弹出时切后台返回终端闪烁」

#### Scenario: 设备在后台丢失时仍发布缓存遮罩

- **WHEN** 切后台前后 Vulkan 设备丢失（2026-11-17 真机日志：切后台时
      `Unknown: Device is lost`）
- **THEN** 宿主 MUST 发布缓存里最近一帧作为开孔遮罩，MUST NOT 因回读失败退回平底色
- **AND** 仪器化用例 `BackgroundResumeFlickerInstrumentedTest#deviceLostInBackgroundStillPublishesStoredCoverFrame`
      MUST 注入设备丢失复现该路径，断言切后台后发布出的遮罩像素不止一种颜色，
      且回前台后重新出帧并撤下遮罩

#### Scenario: 遮罩发布即重绘，且重绘在窗口可见期内

- **WHEN** 运行期在切后台路径上发布开孔遮罩
- **THEN** `SurfaceView` MUST 立即重绘，MUST NOT 等组合层的下一次重组（遮罩发布发生在
      渲染线程，与组合层的失效时机无关）
- **AND** 重绘 MUST 发生在平台拆掉 surface **之前**：那之后的窗口不再有帧，画了也上不了
      屏。实测反向对照（去掉发布侧重绘）序列为 `(true,true) → (false,true) →
      (false,true,painted) → ……`——遮罩只在 surface 已经拆掉之后才画，正好那一帧没人看
- **AND** 每一次重绘覆盖开孔 MUST 回记为「已涂满」（遮罩画面或底色都算），供仪器化
      用例区分「有遮罩」与「遮罩真的上屏」

#### Scenario: 遮罩在画面恢复后撤下

- **WHEN** 新 surface 提交了首帧
- **THEN** 遮罩 MUST 被撤下并释放像素
- **AND** 撤下 MUST 延后到**队列深度足够的显示帧**：`present` 只把缓冲排入 surface 的
      队列，SurfaceFlinger 要到下一次 vsync 才把它锁存上屏；交换链按
      `desired_maximum_frame_latency = 2` 配置，队列可以压两帧才交出去，而应用侧的
      frame 回调与合成器的锁存发生在同一个 vsync、先后无保证。故延后 MUST 覆盖队列
      深度（当前两帧），差一帧撤遮罩，开孔就空一帧——就是「切回前台闪一下」
- **AND** 「无缓冲区」（surface 被拆）MUST 立即生效，遮罩 MUST 马上回来：那条路上
      延迟一帧等于让已经消亡的 surface 再裸一帧
- **AND** 帧提交上报 MUST 带上「渲染开始时」的开孔世代：开孔在渲染途中被拆时，这一帧
      提交的是平台已废弃的缓冲区，MUST NOT 据此撤遮罩——否则新开孔还没有任何缓冲，
      遮罩已被撤掉，那条空档就是闪烁。世代不符的上报 MUST 被丢弃，由新开孔的真实帧
      重新上报（输入法弹出会拆装 surface，渲染线程同时在跑，这是常态而非边角）

#### Scenario: 模拟器实测往返全程不露底

- **WHEN** 仪器化用例 `BackgroundResumeFlickerInstrumentedTest` 在模拟器上走真实
      HOME → `am start` 往返
- **THEN** 该用例 MUST 覆盖两种形态：**键盘收起**（像素条带可比）与**键盘在场**
      （`docs/STATUS.md` 报的场景——开孔被位移/改高，遮罩与开孔尺寸不一致，走等比
      缩放贴合）；键盘在场的用例 MUST 断言往返前后终端位移相同，否则像素比对的不是
      同一批可见行
- **THEN** 判据 MUST 取产品自己的开孔状态转换序列——截图与轮询采样都抓不到一两帧的
      瞬态（模拟器一次截图 >100ms，后台采样线程还会被进程冻结）：断言 MUST 包含
      「无缓冲 + 有遮罩」、「遮罩在 surface 仍在时已涂满窗口」，且 MUST NOT 出现
      「无缓冲 + 无遮罩 + 未涂满」
- **AND** 「本次往返新增的条目」MUST 按记录序号取，MUST NOT 按条数差分：开孔状态
      日志是容量有限的环形缓冲，满了之后往返前后条数相等，按条数差分恒为空，整段
      往返会被算成「什么都没发生」（实测全量套件里即如此判红，红灯指向用例的差分
      口径而非被测的开孔行为）
- **AND** 实测序列 MUST 为 `(true,true) → (true,true,painted) → (false,true) →
      (true,true) → (true,false)`：遮罩在离场时发布并当场画进窗口（surface 还在）、
      平台拆 surface 那一拍遮罩已在窗口里接住、新缓冲上屏后才撤下
- **AND** 回前台首帧 MUST 走原生 `warmup: presenting the retained last frame` 分支，
      MUST NOT 走 `no retained frame to present`
- **AND** 该用例 MUST 做过反向对照，否则它证明不了任何事。反向对照实测（模拟器，
      2026-11-15）：去掉「遮罩发布即重绘」与「暂停期间照画遮罩」两处后，同一用例
      MUST 失败，序列变成 `(true,true) → (false,true) → (false,true,painted) →
      (true,true) → (true,false)`——遮罩直到平台拆掉 surface 之后才画进窗口，
      那一帧没有任何窗口会再看，窗口里留着的仍是离开前的空开孔；去掉这两处后
      修复态的序列不复现，故该断言对这两处改动敏感

#### Scenario: 过渡期的开孔必须直接量到像素

- **WHEN** 仪器化用例 `TerminalHoleTransitionFillInstrumentedTest` 在模拟器上走真实
      HOME → `am start` 往返，并用 `PixelCopy` 连续读**窗口自己的缓冲**
- **THEN** 该用例 MUST 直接读窗口缓冲（不含 `SurfaceView` 的独立图层）：原生 surface
      位于窗口**之下**，截图量到的是「窗口 + 下层 surface」的合成结果，分不清「洞是空的」
      与「洞被填了但填的不是遮罩」
- **AND** 判据 MUST 包含：前台稳态开孔保持透明（遮罩不得盖住活内容）、遮罩发布后开孔
      在 500ms 内被填满、填满发生在平台拆 surface 之前、过渡段空开孔占比不超过 20%、
      原生无缓冲且遮罩在位时开孔不得露出后方
- **AND** 实测值 MUST 为：前台稳态不透明占比 0.000；遮罩发布后 79~98ms 内跃到 1.000；
      过渡段未填满样本占比 1.2%~3.7%
- **AND** 反向对照实测（去掉「遮罩发布即重绘」与「暂停期间照画遮罩」两处）：遮罩发布后
      整个过渡段开孔一直透明（实测 72~88 帧、约 380ms），直到回前台才被填上，填满延迟
      实测 3278ms——那正是用户看到的「切后台时窗口里的洞透出桌面」；该用例 MUST 因此失败

#### Scenario: 键盘在场形态的像素一致性只量应用渲染的画面

- **WHEN** `BackgroundResumeFlickerInstrumentedTest` 的键盘在场形态做往返前后像素比对
- **THEN** 判据 MUST 排除输入法覆盖的行（`contentBand`：`band.bottom` 裁到
      `shot.height − IME inset.bottom`）：键盘上画什么（布局、皮肤、文案）不属于应用
      可渲染的终端画面，同一次往返实测 GBoard 会换皮肤（深色↔浅色），像素差异会漂在
      键盘上而非终端内容上
- **AND** 「键盘已画上屏」的判据 MUST 取 `ImeTracker` 的 `onShown` 锚点（平台的跟踪器，
      与输入法品牌无关）：`rootWindowInsets.isVisible(ime())` 会超前派发——实测请求后
      一拍即上报可见、面板约 3 秒后才画，取它做等待条件会在键盘未画时就放行验证
- **AND** MUST NOT 以「底部条带颜色种数大于阈值」判定键盘是否上屏：平底的 Compose
      区域也足够凑出 8 种颜色，实测会误判（键盘实际未画）
- **AND** 该用例 MUST 保持 `shiftBefore == shiftAfter` 断言，否则像素比对的不是同一批
      可见行

#### Scenario: 切后台返回后的一致性比对等到动画彻底收住

- **WHEN** `ImePopupPixelInstrumentedTest#imeShiftFrozenWhileBackgrounded` 在回前台后
      做「离开前的截图 vs 返回后的截图」像素比对
- **THEN** 比对 MUST 先轮询到回前台帧的条带像素与离开帧一致（15 秒上限，150ms 间隔）
      再断言；亚像素位移低于判据阈值时会被当成「停住」，用「连续两次截图一致」放行
      会量到动画中途帧（实测同一位置稳定时仍差 2000+ 像素）
- **AND** MUST NOT 以「表面屏幕坐标回到离开时的位置」代替收敛判据：坐标一致早于
      面板/键栏与内容的补帧结束（实测坐标一致时像素仍差 2000+）
- **AND** 超时 MUST 带着真实差值落到断言而不是无限期等待或清空比对口径

#### Scenario: 从没有过缓存时不遮

- **WHEN** 冷启动后尚未成功缓存任何一帧（无保留帧或帧格式不是 RGBA8）
- **THEN** 宿主 MUST 沿用平底色遮罩，MUST NOT 以空遮罩代替（空遮罩等于一块黑）
- **AND** 已有缓存时偶发一次回读失败 MUST NOT 清空缓存（见运行期缓存场景）

### Requirement: 设备轮询的失败形态必须归一

wgpu 的 `Device::poll` 有三种失败形态，调用方的 `Result` 只接得住常态那一种：
`PollError::Timeout` 是分步轮询的常态（这一步等满还没排空），`WrongSubmissionIndex`
是提交序号对不上，而设备真出错时 wgpu 内部直接 `panic!`（`handle_error_fatal`）。
那道 panic 发生在 wgpu 内部，会一路穿出 JNI 边界变成未捕获的 Java 异常，直接杀掉
整个进程（2026-11-12 真机日志：切后台时遮罩帧回读撞上设备错误，整进程退出）。

故凡调用 `Device::poll` 的路径 MUST 经统一封装，把结局归一成「继续 / 失败」：
常态继续；真错误与 panic 都归成「失败」并原样带出原因（MUST NOT 吞掉、MUST NOT
改写）；panic MUST NOT 越过封装边界向上传播。

#### Scenario: 等满一步仍未排空不算设备错误

- **WHEN** 设备轮询等满一步仍未排空（`PollError::Timeout`）
- **THEN** MUST 判为「继续」让调用方接着等，MUST NOT 记为设备错误

#### Scenario: 设备轮询内部 panic 不得掀掉进程

- **WHEN** 设备轮询在 wgpu 内部 panic
- **THEN** MUST 判为「失败」并带出 panic 原文，进程 MUST 存活
- **AND** 失败原因 MUST 出现在日志里，调用方据此放弃本次补救并如实上报

### Requirement: 开孔覆盖由两段互补机制承担

开孔的覆盖 MUST 由两段互补的机制承担，MUST NOT 只靠其中一段：

- surface **尚未交付**（新 `SurfaceView` 还没拿到窗口）：平台还没打洞，窗口层的像素
  可见，故 `onDraw` 里画的遮罩（离开时的画面，无画面时是终端底色）能挡住后方桌面；
  窗口背景 MUST 取终端底色，窗口层来不及画的区域也显示终端底色而非桌面。
- surface **已交付但尚无新帧**（交换链重建期间）：洞已经打好，窗口层的像素被抹掉，
  遮罩 MUST 由**原生侧**把保留的上一帧呈现进新 surface 来覆盖；`onDraw` 的遮罩在
  这一段不可见。

#### Scenario: surface 尚未交付时靠窗口层遮罩

- **WHEN** 新 `SurfaceView` 还没拿到窗口，宿主已落回「无缓冲区」状态
- **THEN** 开孔 MUST 显示终端底色或离开时的画面，MUST NOT 透出后方桌面

#### Scenario: surface 已交付但无新帧时靠原生侧呈现

- **WHEN** 新 surface 已交付但尚未提交首帧
- **THEN** 原生侧 MUST 把保留的上一帧呈现进新 surface，MUST NOT 让开孔透出桌面

### Requirement: 冷启动耗时以进程启动为原点被度量

应用 MUST 在进程内度量冷启动耗时并对外暴露：至少包含「进程启动 → 首帧上屏」
与「进程启动 → 首个 PTY 输出」两个口径，后者对应用户口中的「shell 启动慢」
（提示符落格、shell 真的可用）。

度量 MUST 以**进程启动时刻**为原点，MUST NOT 由测量方在事后用「当前时刻 −
进程启动时刻」倒算：测量方通常在应用启动之后很久才运行，那样的差值是「测量方
跑了多久」，与启动耗时无关。

#### Scenario: 首帧上屏后度量可读

- **WHEN** 终端已向 Surface 呈现第一帧
- **THEN** 「进程启动 → 首帧」的毫秒数可被读取，且不为 null

#### Scenario: shell 首次输出后度量可读

- **WHEN** 终端收到首个 PTY 输出（提示符落格）
- **THEN** 「进程启动 → 首个输出」的毫秒数可被读取，且不为 null

#### Scenario: 尚未发生时不谎报

- **WHEN** 首帧尚未呈现、或 shell 尚未输出
- **THEN** 对应度量保持为 null，MUST NOT 报 0 或其他占位值

#### Scenario: 同一会话的首个输出只上报一次

- **WHEN** 渲染线程因切后台／换 surface 重启，或同一会话继续输出
- **THEN** 「进程启动 → 首个 PTY 输出」的度量 MUST 仍是**首次**输出的时刻，
      `first output received` MUST NOT 被重复记录；判定 MUST 挂在会话条目上
      （渲染线程重启会重建循环内的局部标志）

### Requirement: 启动屏与桌面图标共用单一图案源

系统启动屏的图标 MUST 是**普通图形**（`@drawable/splash_icon`），MUST NOT 直接给自适应
图标。顶层 drawable MUST 是 `Animatable`（现为单帧 `AnimationDrawable`）：平台对非
`Animatable` 的自定义图标先按 `starting_surface_default_icon_size`（108dp）光栅化，再
放大到 192dp 单元（1.78×），矢量边缘会被重采样成模糊。

彩色图案（终端窗口 + 星形 + 点迹）在仓内 MUST 只有一份（`@drawable/logo_art`）：启动屏
与桌面图标前景都只包装引用它，MUST NOT 各自维护路径副本。

启动屏包装 MUST 是固定尺寸的收缩包装，其尺寸按图案的预缩放比例放大；其墨迹半径 MUST
严格小于平台蒙版安全半径且不小于其 40%。

#### Scenario: 平台启动屏几何（动机）

- **WHEN** 平台冷启动绘制启动屏图标
- **THEN** 图标单元 = `starting_surface_icon_size`(160dp) ×
  `splash_icon_no_background_scale_factor`(1.2) = 192dp，蒙版按 `config_icon_mask`
  缩放，安全半径下界 = 单元的 50/108 ≈ 88.9dp；裸 `layer-list`（无固定尺寸子项）会被拉到
  满单元（放大 1.78×），墨迹半径超越蒙版上限，星尖与窗口一角被裁——用户报「图案不完整，
  被限制到图标形状」的实测根因，也是本收缩包装的动机

#### Scenario: 矢量启动图不被光栅化

- **WHEN** 平台为 `windowSplashScreenAnimatedIcon` 创建绘制包装
- **THEN** 顶层是 `Animatable` 时走 `AnimatableIconAnimateListener`，每帧直接绘制矢量；
  MUST NOT 落入 `ImmobileIconDrawable` 的 108dp 位图预绘制路径
- **AND** 启动屏图案在 420dpi 下按 192dp 单元直接栅格化，窗口描边与星尖保持矢量锐度，
  MUST NOT 出现 108dp→192dp 双线性放大造成的模糊

#### Scenario: 实测口径

- **WHEN** 验证启动屏输出
- **THEN** Robolectric 模型与平台 emulator 录屏的墨迹半径均 ≈0.86×蒙版，
  均在 [40%, 95%] 区间内；旧实现 ≈1.0× 即被裁
- **AND** emulator 420dpi 截图中图案描边的亮度过渡为 2px，与同一屏上的文字抗锯齿一致；
  修复前为 13–20px（108dp 位图放大）

#### Scenario: 启动屏为图形加底色

- **WHEN** 冷启动显示系统启动屏
- **THEN** 只见图形本体画在主题底色上，图标后不出现圆形底板

#### Scenario: 启动屏直接给自适应图标

- **WHEN** `windowSplashScreenAnimatedIcon` 指向 `@mipmap/ic_launcher`
- **THEN** 图案沿桌面蒙版被裁成图标形状，MUST NOT 如此配置
- **AND** 自适应图标的 monochrome 层（`launcher_monochrome`）是平台 themed icon 的单色
      简化重绘，不属仓内那份彩色图案的副本

#### Scenario: 与桌面图标同源

- **WHEN** 审查启动屏图标与自适应图标前景
- **THEN** 启动屏与桌面图标前景都只引用同一份的 `logo_art`，仓内无第二份路径副本；
  图案按安全区预缩放后，桌面前景以 `fill` 呈现、启动屏以固定尺寸收缩包装呈现

### Requirement: 启动图标图案按自适应图标安全区摆放

图案本身 MUST 按平台自适应图标的安全区摆放：整幅图案到画布中心的最大距离 MUST 不超过
视口半径的三分之一（108 视口下即 36 单位），使图案在桌面图标里完整可见。桌面自适应图标
前景（`launcher_foreground`）MUST 以 `gravity="fill"` 引用 `logo_art`，MUST NOT 写死 dp
尺寸——平台把图层区域撑到图标边界的 1.5 倍，写死尺寸会让图案不随图标大小缩放，密度越高
裁得越狠。monochrome 层 MUST 按同一比例预缩放，其路径 MUST 自带起点（弧线直接起头会
画成一整块色斑）。

#### Scenario: 桌面自适应图标的安全区

- **WHEN** 平台按 `AdaptiveIconDrawable` 绘制桌面图标
- **THEN** 图层区域 = 图标边界的 1.5 倍且居中，蒙版圆半径 = 图标边界的一半，安全区圆
  再乘 `66/72`；前景图案的墨迹半径占图层区域的比例 MUST 不超过 `66/72 ÷ 1.5`，
  否则星形与尾迹方块被整块裁掉（实测修复前桌面只剩终端窗口的一角与一个绿点）

#### Scenario: 图案完整可见

- **WHEN** 桌面图标在 48/108/216dp 三档尺寸下渲染
- **THEN** 图案的墨迹半径 MUST 落在安全区内，且星形等细笔画 MUST 有实色像素留存
  （修复前 48px 下橙色星形实色像素为 0，整颗星被裁掉）

### Requirement: 启动屏主题配置与回归防线

`Theme.Terminal.Splash` 的 `windowSplashScreenAnimatedIcon` MUST 指向
`@drawable/splash_icon`；父主题 MUST 为 `Theme.SplashScreen` 且不声明
`windowSplashScreenIconBackgroundColor`。回归防线 `SplashIconRobolectricTest`
MUST 解析平台几何后断言墨迹与蒙版关系，并断言顶层 drawable 是 `Animatable` 的单帧
包装，MUST NOT 只断言资源存在。

#### Scenario: 主题配置

- **WHEN** 审查启动屏主题
- **THEN** 渲染期解析出的 `windowSplashScreenAnimatedIcon` 指向 `splash_icon`，
  顶层是单帧 `AnimationDrawable`，底色 alpha 为 0（无图标背板）

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

#### Scenario: 恢复先稳定若干帧再清零预算

- **WHEN** 宿主因失效换上新窗口，原生 `attach_surface` 成功后失效位回落为 0
- **THEN** 宿主 MUST 保留「恢复确认中」（`surfaceRecoveryPending`），
      新窗口连续出帧达到 `SURFACE_RECOVERY_HEALTHY_FRAMES`（60，60fps 下约 1 秒）
      之前 MUST NOT 清零换视图预算
- **AND** 每次新窗口挂载 MUST 把稳定计数从头开始（只统计当前窗口），
      但 MUST NOT 清除「恢复确认中」——刚挂上的窗口尚未证明不是下一具尸体
- **WHEN** 连续稳定帧数达到阈值
- **THEN** 预算 MUST 整条复位（`SurfaceRecreateBudget()`），恢复确认状态 MUST 清零，
      新 surface 重新接受快试判定
- **AND** 稳定计数以帧数累计，MUST NOT 依赖挂钟与周期帧节拍

#### Scenario: 单帧成功不能清零预算

- **WHEN** 2026-11-23 11-05-03 真机日志（Mali-G57）在同一冷启动会话内出现 14 次设备丢失
      （14 条 `GPU device lost`、14 次重建成功、15 代设备）：前 5 次快试的每一代新设备
      都在挂载后 14–20ms（首帧后 6–15ms）再次丢失；11:03:03 预算再次被单帧成功清到
      `attempt 1/5`，随即重演 5 次快试，直到 11:03:05.9 重建的一代存活
- **THEN** 单帧成功 MUST NOT 清零预算，只有连续稳定帧达到阈值才复位；
      本样本的 14 次丢失 MUST 记为有界恢复，MUST NOT 归因为重建动作缺陷
- **AND** 应用侧 MUST 如实记录：2026-11-22 18-04-41 与 2026-11-23 11-05-03 两簇设备
      丢失的底层根因（Mali-G57 在首帧后丢失）未能定位，有界恢复是缓解而非消除，
      MUST NOT 在规范中表述为「已解决」

#### Scenario: 重建成功清零失效位

- **WHEN** 重建成功或 `detachWindow` 释放 surface
- **THEN** 原生失效位 MUST 清零，宿主侧稳定计数 MUST 从头开始，
      新 surface 重新接受判定；「恢复确认中」只能由稳定出帧或回到前台解除
      （见上一条场景）

#### Scenario: 失效位随打包返回上报

- **WHEN** 原生完成一帧渲染
- **THEN** 失效位 MUST 置于打包返回的第 53 位，MUST NOT 挂在渲染计数门下
      （空闲帧也会读取）

#### Scenario: 宿主换视图而非重挂同一窗口

- **WHEN** 宿主观察到失效位为 1
- **THEN** MUST 换掉整个 `SurfaceView` 以取得**新的**原生窗口；
      MUST NOT 仅对同一窗口反复 detach/attach（唤不活被遗弃的 BufferQueue）

#### Scenario: 重建请求限流

- **WHEN** 失效位持续为 1
- **THEN** 前 `SURFACE_RECREATE_MAX_ATTEMPTS` 次请求间隔 MUST ≥ 最小间隔（500ms）；
      用尽后 MUST 转为慢重试（间隔 ≥ 5s）并只告警一次，MUST NOT 永久停止请求——
      平台迟迟不交付新窗口时，停止请求等于把渲染线程钉在死窗口上，用户看到永久空白

#### Scenario: 换视图预算不得随每次挂载清零

- **WHEN** 同一前台会话内连续发生设备丢失并触发多次 `surfaceCreated`/`resumeRendering`
- **THEN** 预算 MUST NOT 随每次 `surfaceCreated`/`resumeRendering` 清零；
      复位只允许两种时机：`resumeRenderingFromBackground`（`ON_RESUME`），
      或新窗口连续稳定出帧达到 `SURFACE_RECOVERY_HEALTHY_FRAMES`
      （见「恢复先稳定若干帧再清零预算」）
- **AND** 否则每次 attach 都把 5 次快试上限与 500ms 最小请求间隔一并重置，限流永不生效：
      实测 2026-11-19 设备丢失两簇共 15 次（17:44 簇 10 次、17:46:03 簇 5 次），每次都
      换掉一个 `SurfaceView` 并重建一次 `VkInstance`，请求全部记为 `attempt 1/5`；17:44 簇
      1.8s 内 10 次、17:46:03 簇 0.8s 内 5 次，均约 190ms 一次，既拖慢恢复又给驱动施加压力
- **AND** 应用侧 MUST 如实记录：本次设备丢失的底层根因（Mali-G57 两簇丢失）未能定位，
      应用配置（`PowerPreference`／present mode／`ANativeWindow_setBuffersGeometry`）
      均无证据为成因；有界恢复是缓解而非消除，MUST NOT 在规范中表述为「已解决」

### Requirement: 设备丢失后停止向死设备提交

Vulkan 设备被判死（`VK_ERROR_DEVICE_LOST`）后，MUST NOT 继续向死设备提交调用：
wgpu 在致命 HAL 错误上先调设备丢失回调、再 panic，逐帧调用会逐条复现同一条 panic，
而宿主换的新窗口仍挂在同一个死设备上，画面永久黑屏。

进程的判死位 MUST 按**设备世代**归属：每个设备有与它同生共死的判死标志
（实现见 `native/src/render/context/gpu.rs` 的 `mark_device_lost`）。丢失回调、
未捕获错误与设备轮询只置位它们所属的那个设备；仅当它仍是槽位里的当前设备时，
才置位全局 `DEVICE_LOST` 并记一条 error。旧设备迟到的回调或残留回读失败
MUST NOT 把重建后的新设备判死（否则一次丢失会被放大成连续多轮重建）。

仪器化注入也只作用于当前设备。丢失的第一次记账 MUST 只记一条 error，余波
MUST NOT 逐条重复；旧世代的结局记 debug，如实保留原因，不吞。

#### Scenario: 丢失后不再逐帧 panic

- **WHEN** 设备被判死（丢失回调触发，或取纹理 panic 载荷含 `Parent device is lost`）
- **THEN** 本进程只记一条 error，后续帧 MUST 在 GPU 调用前返回，MUST NOT 逐帧复现 panic

#### Scenario: 丢失余波不得再记 error

- **WHEN** 设备丢失已记一条 error，后续帧、度量查询与回读继续失败
- **THEN** 被 `poll_outcome` 接住的 `Device::poll` panic MUST 由 panic 钩子记 **debug**
      （全文保留，MUST NOT 过滤 tag）；`render skipped` MUST 记 **warn** 并限频；
      `cell metrics query skipped` MUST 记 **debug**
- **AND** 同一事件 MUST NOT 产生第二条 error（规范「只记一条 error」）

#### Scenario: 旧设备迟到的丢失不得判死新设备

- **WHEN** 设备已被重建替换，旧设备的丢失回调或残留回读轮询随后失败
- **THEN** MUST NOT 置全局丢失位，MUST NOT 触发又一轮换窗口/重建，新设备照常出帧；
      旧世代的失败原因以 debug 记录

#### Scenario: 丢失后所有 GPU 入口先于 wgpu 返回

- **WHEN** 进程已判定设备丢失
- **THEN** `render_inner` MUST 返回 0（空闲），`read_cover_frame` MUST 返回 `None`，
      `attach_surface` MUST 直接放弃；设备轮询 MUST 在被轮询设备自己的判死位已置位时
      直接给出失败结局、MUST NOT 再碰死设备（wgpu 在致命 HAL 错误上先调丢失回调、再
      panic，回调之后任何 `Device::poll` 都只会换来一道 panic）；宿主 MUST 发布运行期
      缓存的遮罩，只有从未缓存过任何帧时才退化为平底色

#### Scenario: 丢失判据不以错误类型为唯一条件

- **WHEN** 丢失回调未触发，丢失只体现在 panic 载荷或错误文字里
- **THEN** MUST 同样置位并只记一条 error，MUST NOT 因路径上没有 `DeviceError` 而漏判

### Requirement: 换新窗口触发 GPU 重建并恢复渲染

恢复 MUST 走「宿主换新 `SurfaceView` → `attachWindow` 重建 instance/adapter/device 与
`Renderer` → 清除丢失位 → 正常出帧」这一条链，MUST NOT 依赖进程重启。

`renderWithNewOutput` 的失效位 MUST 把 `device_lost` 与 `surface_invalidated` 合并上报
（同一动作：宿主换新窗口）；否则丢失后宿主等不到任何换窗口触发。重建 MUST 发生在收到
**新** `ANativeWindow` 的 `attachWindow` 内，且先于 `attach_surface`：`wgpu::Surface`
绑定 instance，旧 surface 不能配到新设备。

重建 MUST 搬运与设备无关的会话状态（暂停位、背景色、滚动偏移），MUST NOT 让恢复后的
首帧丢背景色或跳回顶部；重建失败 MUST 保持失效位并释放窗口引用，由宿主的重建预算继续
换窗口。

#### Scenario: 新窗口触发重建并恢复

- **WHEN** 丢失位已置且宿主送来新的 `ANativeWindow`
- **THEN** MUST 重建 instance/device 与 `Renderer`，清除丢失位，
      首帧 MUST 恢复出图（暂停位、背景色、滚动偏移保持）

#### Scenario: 重建失败继续换窗口

- **WHEN** 重建 instance/device 失败
- **THEN** 丢失位 MUST 保持，窗口引用 MUST 释放，失效位 MUST 继续上报，
      宿主按重建预算继续换窗口

#### Scenario: 恢复后新建会话照常出帧

- **WHEN** 设备丢失恢复后宿主新建会话
- **THEN** 新会话 MUST 复用已重建的 instance/device 并照常出帧，
      MUST NOT 挂到丢失前的死设备上（`SurfaceLossRecoveryInstrumentedTest`
      在恢复后新建会话并断言墨迹）

#### Scenario: 冷启动首帧后的设备丢失按有界恢复自愈

- **WHEN** 2026-11-20 12-45-04 真机日志（Mali-G57）出现冷启动首帧后的设备丢失：
      `first frame presented`（12:38:40.352）→ `GPU device lost`（12:38:40.357，
      首帧后 5ms；该条 error 由设备丢失回调在非渲染线程 tid 7885 上记出，
      渲染线程 tid 7887 同期只记了首帧呈现与随后的 `render skipped`）→ `render skipped`
      （12:38:40.369）→ `surface invalidated (attempt 1/5)`（12:38:40.370）→
      新窗口 `attachWindow`（12:38:40.476）→ `GPU rebuilt after device loss`
      （12:38:40.538）→ 新 surface 挂载（12:38:40.618）
- **THEN** 该次丢失 MUST 按有界恢复链自愈（换新 `SurfaceView` → 重建 instance/device →
      恢复出帧），从丢失到恢复出帧实测 261ms，MUST NOT 依赖人工干预或进程重启
- **AND** 本次丢失的底层根因 MUST 记为未定位：它发生在冷启动的正常渲染路径上
      （非切后台/换窗簇），应用配置（`PowerPreference`／present mode／
      `ANativeWindow_setBuffersGeometry`）仍无证据为成因；恢复是有界缓解而非消除，
      MUST NOT 表述为「已解决」

#### Scenario: 冷启动连续多次设备丢失仍按有界恢复收敛

- **WHEN** 2026-11-22 18-04-41 真机日志（Mali-G57）在同一冷启动会话内连续出现 3 次
      设备丢失，且第 2、3 次丢的是上一次重建刚建好的**新**设备：
      `first frame presented`（18:00:16.592）→ 6ms 后第 1 次丢失 → 新窗口
      `attachWindow`（16.850）→ `GPU rebuilt after device loss`（16.926）→ 新 surface
      挂载（17.010）→ 28ms 后第 2 次丢失 → `attachWindow`（17.160）→ 重建（17.194）
      → 挂载（17.268）→ 19ms 后第 3 次丢失 → `attachWindow`（17.666）→ 重建（17.691）
      → 挂载（17.772），此后到日志结束未再丢失
- **THEN** 每次丢失 MUST 各自记一条 error：判死位按设备世代归属，新一代设备是新的一份，
      MUST NOT 因上一代已记账而吞掉新世代的丢失
- **AND** 恢复 MUST 仍走既有的「换新窗口 → 重建 instance/device → 挂载」链：每次从丢失
      到新 surface 挂载实测 412ms／230ms／485ms，MUST NOT 依赖人工干预或进程重启
- **AND** 换视图预算 MUST NOT 因连续重建而复位：本样本 3 次记为 `attempt 1–3/5`，
      限流（快试 5 次 ≥500ms）继续生效
- **AND** 该样本 MUST NOT 被归因为「应用重建动作有缺陷」：每一代新设备都独立判死，
      重建链本身未出现失败（3 次 `GPU rebuilt after device loss` 全部成功）；
      底层根因 MUST 继续记为未定位，有界恢复是缓解而非消除，MUST NOT 表述为「已解决」

### Requirement: 遮罩回读不得以阻塞等待换取映射完成

遮罩回读（`read_cover_frame`）在非渲染线程上等自己的拷贝完成时，MUST 用
`PollType::Poll` 分步轮询，MUST NOT 用 `PollType::Wait`：后者的有限超时在 Vulkan
后端落到 `vkWaitSemaphores`，而 Mali-G57 上这条带超时的等待本身会返回
`VK_ERROR_DEVICE_LOST`。

回读 MUST 只在本设备世代已出过至少一帧后放行：重建保留的累加器内容是上一代设备的。

回读的「提交 + 注册映射」MUST 与轮询同处设备丢失归一范围内：判死位到 wgpu panic
之间仍有一瞬，`Queue::submit` 与 `BufferSlice::map_async` 在这个窗口里撞上
`Parent device is lost` 会穿出 JNI 杀掉进程。两者 MUST 由同一道归一覆盖，MUST NOT
只包住提交再逐调用补判死。

#### Scenario: 2026-11-24 真机日志的丢失循环

- **WHEN** 2026-11-24 17-40-54 真机日志（Mali-G57）在 14.6s 内出现 8 次设备丢失，
      每次丢失前最后一条 GPU 调用都是非渲染线程的 `device poll`，且每次都在
      `attach_surface: configured` 之后 1–14ms（首帧呈现后 5–20ms）
- **THEN** MUST 归因为回读的阻塞等待：回读线程等在 `vkWaitSemaphores` 上，首帧后
      数毫秒内返回丢失，重建后的第一次回读再次触发，形成「重建→丢失」循环
- **AND** 处置 MUST 是上述非阻塞轮询与「出过帧才回读」，MUST NOT 以过滤日志、
      降级 error 或跳过回读功能来掩盖
- **AND** 底层驱动缺陷 MUST 继续记为未定位：本修复移除的是应用侧触发条件，
      MUST NOT 表述为「Mali-G57 的设备丢失已解决」；须在同一设备上重新采日志确认
      丢失不再出现

#### Scenario: STATUS.md 五类症状与根因的对应

- **WHEN** 核对 `docs/STATUS.md` 登记的「无法滚动／卡顿／终端页面多次死亡／抽搐／
      诡异自动翻动滚动闪屏」（该文件禁止修改，结论只落在此处）
- **THEN** 每条 MUST 对应到具体根因与可复核的判据，MUST NOT 停留在症状描述：
      - 「终端页面多次死亡」= 设备丢失后的换窗重建循环：同一前台会话内
        `GPU device lost` 与 `surface invalidated (attempt N/5)` 1:1 连发，
        且每次重建后 1–14ms 再丢（17-40-54：14.6s 内 8 次，快预算 5 次后转 5s 慢重试）
      - 「卡顿」「无法滚动」= 两个独立来源：重建期间渲染线程反复停在暂停位
        （`setRenderPaused: paused=true` 连发）；滚动期间渲染循环塌到 500ms idle latch
        （见 `render-loop-scroll-cadence`）
      - 「抽搐／诡异自动翻动滚动闪屏」= 新输出贴底只清整行偏移、不清亚行余量，
        以及重建后交换链内容跳变（见 `render-loop-scroll-cadence` 的「新输出贴底
        必须同拍清掉亚行余量」）
- **AND** 这些根因的设备丢失底层驱动部分 MUST 继续记为未定位：判据只能证明应用侧
      触发条件已移除，MUST NOT 表述为「五类症状已由代码修复验证」
- **AND** 验证 MUST 取同一 Mali-G57 设备的新日志核对丢失次数、`SLOW_*` 条数与
      重建次数三者；三者与预期不符时 MUST 重新归因，MUST NOT 沿用本结论

#### Scenario: 2026-11-25 同设备新日志的复核结果

- **WHEN** 用 2026-11-25 的 `09-45-38` 与 `09-47-15` 核对上条的三个判据
- **THEN** `device lost`／`Parent device is lost`／`readCoverFrame` panic／
      `surface invalidated` 均为 0 条，重建只出现 `attach_surface: ANativeWindow
      changed`（5 次）与 `RECONFIGURE_SWAPCHAIN`（4 次）这类正常换窗路径
- **AND** `SLOW_*` 共 3 组，分别落在首帧管线建立与首次新输出，未在稳态循环里成片出现；
      「卡顿」的判据因此 MUST 改写为「启动与首次内容的瞬时帧」，MUST NOT 继续
      记作持续掉帧
- **AND** 滚动判据：`scrollback` 从 0 增长到 245 行后仍正常记账，未出现 idle latch
      塌缩；「无法滚动」与「诡异自动翻动」的新日志证据为 0，MUST NOT 据此宣称
      五类症状已全部由代码验证，只能记为「本批日志未复现」

#### Scenario: 五类症状的可复现判据

- **WHEN** 需要复核 `docs/STATUS.md` 的五类症状（该文件禁止修改）
- **THEN** 每类 MUST 对应到可单独运行的测试与日志判据：
      - 无法滚动：`ScrollDistanceTest`（越界回弹、惯性逐帧落点）与
        `ScrollBehaviorQuantifiedTest`（模拟器实测位移与内容一致）
      - 卡顿：`FrameTimingStatsTest`、`DisplayPeriodEstimateTest` 与
        `RenderPauseTest`；日志判据是稳态循环不出现成片 `SLOW_LOOP`
      - 终端页面多次死亡：`ReadCoverFrameSafelyTest`、`SurfaceRecoverySettledTest`、
        `SessionLivenessTest` 与 `SurfaceLossRecoveryInstrumentedTest`；日志判据是
        无 `device lost`／`surface invalidated` 连发
      - 抽搐：`ModifierBarGestureAreaTest` 的抖动半径与
        `TerminalSurfaceLogicTest` 的捏合落定，均断言小位移不触发状态跳变
      - 诡异自动翻动／闪屏：`ScrollRemainderTest` 的「新输出贴底时亚行余量与整行偏移
        同拍清零」与 `ImeSurfaceStabilityInstrumentedTest` 的像素稳定性
- **AND** 复核 MUST 运行上述测试并附结果；只凭日志中未复现 MUST NOT 判定症状已消除
- **AND** 最终树的本地全量 `:app:connectedDebugAndroidTest` MUST 为 262/262 通过
      （0 failed、0 skipped）；CI 侧 262 个仪器化与 137 个 benchmark 用例同样 MUST 通过

#### Scenario: 回读阻塞等待的引入与回归来源

- **WHEN** 复核 2026-11-17 当日的遮罩回读变更：先因设备丢失**移除**回读，13 小时后
      又以「切后台返回闪烁」为由**恢复**，恢复时用带超时的阻塞等待分步等自己的拷贝
- **THEN** 该恢复 MUST 被记为 2026-11-17 至 2026-11-24 多份日志中设备丢失循环的来源：
      回读在每次表面挂载后数毫秒内在 `vkWaitSemaphores` 上返回 `VK_ERROR_DEVICE_LOST`，
      宿主据此换窗重建，新设备再被同一条等待打死
- **AND** 结论 MUST 是「恢复功能时没有一并评估被移除时的成因」，MUST NOT 说成
      「回读功能本身有害」：回读保留，只把等待换成非阻塞轮询、并加「本设备已出过帧」
      的前置条件
- **AND** 任何以「先移除功能」收场的修复 MUST 在同一规范里留下该功能的恢复条件；
      恢复时 MUST 重新核对当初的成因是否已被处理

#### Scenario: 丢失后的预热失败不算新的 error

- **WHEN** 设备已判死，宿主触发的 `prefetchRenderState` 因 GPU 不可用而失败
- **THEN** MUST 记 **debug**（如实保留原因），MUST NOT 记 error：它随后必被宿主发起
      的重建取代，按「丢失余波不得再记 error」处理

### Requirement: 判死的设备不得被复用

判死后的重建 MUST 换掉判死的那个设备：全局槽位里若还是判死的设备，MUST 新建
instance/adapter/device 并换上，MUST NOT 复用它——Vulkan surface 绑定 instance，
旧 instance 上的 surface 配不到新设备，复用等于把黑屏原样留着。

#### Scenario: 判死的设备不得被复用

- **WHEN** 重建路径被调用，而全局槽位里那个设备已判死
- **THEN** MUST 新建 instance/adapter/device 并换上，MUST NOT 复用判死的那个；
      另一线程已重建好时 MUST 复用同一个设备，MUST NOT 各建一个

### Requirement: 不得以日志手段掩盖设备丢失

MUST NOT 以反射清平台同步组、降级日志等级、过滤 tag 或缩短日志来掩盖设备丢失；丢失的
第一次记账必须是 error，余波的收敛靠「停止向死设备提交」，不是靠沉默。

#### Scenario: 丢失的第一条记录是 error

- **WHEN** 进程内首次判定设备丢失
- **THEN** MUST 记一条 error 级别的日志，MUST NOT 以降级等级、过滤 tag 或缩短日志代替

### Requirement: 已取出的帧不得因 surface 竞态丢弃

`receive_cell_data` 是破坏性取数，VT 去重基线在入队时已推进，被取走又丢弃的帧不会补发。
渲染 MUST 在真正提交前复查 surface：若 surface 在取帧之后才被拆除，本帧单元数据 MUST
存入 `last_frame`，并 MUST 解除滚动在途门控；重新挂载后的空闲重绘据此补画，MUST NOT
让屏幕停在移除前的旧内容或永久黑屏。

启动进行中视图重建时，新 Surface 只经启动入口抵达运行期。该入口 MUST 先登记
待绑定 Surface、再决定是否新建会话：早退只表示「本次不新建会话」，MUST NOT
把一并抵达的 Surface 丢弃——丢弃后正在进行的启动绑定的是旧（已销毁）Surface
或空，终端从此没有画面且无自愈。

判活与初始网格尺寸 MUST 取登记处里的最新一张，MUST NOT 取启动入口的入参：
入参那张在启动期间换窗后早已作废，拿它判活会让整条启动中止，按它的尺寸
spawn 会让 PTY 初始 winsize 与实际窗口不符。

#### Scenario: 取帧与 surface 拆除并发

- **WHEN** 帧数据已从 VT 取出，进入渲染阶段时发现 surface 为空
- **THEN** 该帧 MUST 写入 `last_frame`、滚动在途标志 MUST 清零，
      下一次挂载后的空闲重绘 MUST 呈现该帧内容

#### Scenario: 启动期间抵达的新 Surface 不得被丢弃

- **WHEN** 会话正在启动，而此时抵达了一张新的 Surface
- **THEN** 该 Surface MUST 被登记为待绑定项；正在进行的启动 MUST 绑定它，
      MUST NOT 绑定已销毁的旧 Surface 或空

#### Scenario: 换窗后判活与网格尺寸取最新一张

- **WHEN** 启动期间 Surface 已换新
- **THEN** 判活 MUST 判新那张是否有效，spawn 的初始 winsize MUST 按新那张的尺寸计算

### Requirement: 空闲重绘失败必须归还内容脏标志

空闲路径是字体、主题等「只置 `dirty`、不产生新帧」改动的唯一送达通道。该标志在被
`swap(false)` 消费后若渲染失败，MUST 立即重新置位，MUST NOT 让改动永久丢失；缓存不可用
（无 `last_frame` 或会话不一致）时 MUST NOT 消费该标志。

#### Scenario: 主题变更后空闲重绘失败

- **WHEN** 主题变更只置 `dirty`，随后空闲重绘因 GPU 错误失败
- **THEN** `dirty` MUST 保持置位，恢复后的下一帧 MUST 重试并呈现新主题

### Requirement: 切换会话后呈现新会话的整幅内容

累加器与 `last_frame` 缓存只属于产帧的那个会话。切到另一会话后，渲染 MUST NOT 停在
上一会话的像素上等它恰好产生新输出：缓存帧会话与当前会话不一致时，渲染路径 MUST 主动
请求该会话按当前状态重推一帧，且该请求 MUST NOT 被内容去重吞掉（切会话时终端本身毫无
变化）；该帧到达后 MUST 整幅重画。

#### Scenario: 空闲会话被切回

- **WHEN** 会话 A 已有内容且此后无新输出，用户从会话 B 切回 A
- **THEN** 渲染路径 MUST 请求 A 重推一帧，A 的整幅内容 MUST 在无需任何输入的情况下
      呈现（脏掩码覆盖全部行，`presented … bands=1`），MUST NOT 保持 B 的像素

#### Scenario: 重推请求不被内容去重吞掉

- **WHEN** 会话内容与上次推送逐字节相同（终端毫无变化）而收到重推请求
- **THEN** VT 线程 MUST 失效去重基线并推送该帧

#### Scenario: 无会话可推时不报错

- **WHEN** 缓存帧属于别的会话而当前会话已在注册表中消失
- **THEN** 请求 MUST 静默跳过（返回失败而非 panic），渲染照常返回「本帧无内容」

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
MUST NOT 因此改变。备用屏状态 MUST 随每帧渲染结果一并上报（与内容下沿同批），MUST NOT 由输入法定居后的独立查询提供：输入法弹出动画期间该状态会翻转
（启动 helix 的同时键盘正收起），查询所得缓存必然滞后于当帧的位移计算。

#### Scenario: 以网格收缩兑现适应窗口大小

- **WHEN** 备用屏下输入法弹出且终端位移为 0
- **THEN** 网格高度 MUST 扣除输入法遮挡并防抖后下发一次 SIGWINCH，MUST 只重算网格、MUST NOT 重配交换链；主屏 MUST NOT 扣遮挡

**位移为 0 MUST 同时以网格收缩兑现「适应窗口大小」**：窗口是 `adjustNothing`，
Surface 尺寸全程不变，故备用屏下 MUST 把输入法遮挡计入网格高度
（`rows = floor((surface − ModifierBar − 输入法遮挡) / cellHeight)`），使全屏 TUI
收到 SIGWINCH 并按可见高度重绘。仅位移为 0 而网格不变会让键盘遮住的末行
（helix/vim 的状态行即在其中）永久不可见。该重排 MUST 防抖到输入法高度稳定后执行
（MUST NOT 逐帧发 SIGWINCH），MUST 只重算网格，MUST NOT 重配交换链。主屏 MUST
不扣输入法遮挡：它靠位移跟随，改网格会带来重排闪烁与底部行丢失。

该扣减 MUST 走运行期的**单一来源**（`TerminalRuntime.imeGridReserve`），三条网格
重算路径——字号/字族变化触发的 `recomputeGridFromFontMetrics`、视图尺寸变化的
`recomputeRowsColsImmediate`（含度量未就绪时的兜底反推）与 `applyGridResize`——
MUST 共用它。任一条漏扣都会让备用屏在改字号或捏合缩放后被撑回被键盘遮住的高度，
且此后无触发点自愈。离开备用屏时 MUST 同样执行一次重排（此时扣减量为 0，网格按
整屏高度复原）：PTY 停留在被输入法缩小后的行数会让 shell 按残缺网格排版且不自愈。
该重排 MUST NOT 因此在主屏暂停渲染——键盘动画期间平台逐帧派发 insets，暂停会被
一直续到动画结束，终端停止上帧。

输入法遮挡高度 MUST 取自平台 insets 派发（`ViewCompat.setOnApplyWindowInsetsListener`
装在终端 Surface 上）。此前实现是两条合成通道——Compose `WindowInsets.ime` 叶节点
与轮询 `rootWindowInsets`，各自与对方取最大值——该组合的真正问题是在 insets 遍历内
写组合状态：写入不保证被观察到（实测每 300ms 写一次，20 次才换来一次重组），
位移因此从未发生。派发是平台自己的分发路径，已挂载视图必然收到。
键盘可见性翻转时关闭选区手柄与菜单的逻辑 MUST 放在该监听器内，MUST NOT 放在
`onApplyWindowInsets` 重写里：框架在视图装了 `OnApplyWindowInsetsListener` 后
只调它而不再调用重写方法，放在重写里即静默失效。

**终端 Surface 的跟随位移 MUST 由视图自身的 `translationY` 承担，MUST NOT 经组合容器
平移**：组合平移要经「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer 帧回调
里跑；主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时，重组滞后可达十几秒（实测
每 300ms 写一次组合状态，20 次才换来一次重组），期间键盘已弹出而终端内容与键栏纹丝
不动。`translationY` 在 insets 派发的同一拍内生效。键栏是组合覆盖层，只能读同一个
`imeInsetFlow` 上移——它 MUST NOT 另取来源，也 MUST NOT 与终端 Surface 平移两次。
两者取值同源故必然一致；生效时刻不同（`translationY` 同一拍内生效，键栏要等一次
重组），键盘动画期间终端先于键栏上移。

换活动会话 MUST 把内容下沿与备用屏状态重置为新会话的现值；活动会话归零（最后一个
会话被关闭、启动失败回滚）时 MUST 一并复位为两者的缺省值。渲染循环只在**变化时**
发布这两项，新会话首帧若与它自身的字段相同（如备用屏的初始 `false`）就不会发布，
运行期会一直保留**上一个**会话的值。备用屏标记陈旧为真时，输入法扣减会一直扣着
一份早已撤走的键盘高度（网格永久塌缩且不自愈）、主屏位移恒为 0，且输入法动画会在
主屏上白白领走一次渲染暂停。

遮挡高度的**发布者** MUST 唯一，且离场方 MUST 按发布者（而非按数值）归零：同一
窗口里的新旧 Surface 看到同一个输入法高度，数值天然相同，按值判断会把新 Surface
刚发布的真实高度误当成「自己还是最后一个发布者」而覆盖成 0；此后新 Surface 的
「值变化才发布」门控永不触发，运行期永久停在 0（键栏错位、备用屏网格恢复整屏行数
且不自愈）。

平移量的每个输入变化 MUST 触发重算：输入法遮挡、视口尺寸（旋转，Activity 声明
`configChanges` 不重建）、单元格度量（字号与捏合缩放）、内容下沿、备用屏状态。
订阅 MUST 在每次 attach 时重建（detach 已停掉它们，而视图复用不重走 viewModel 注入）。

渲染暂停 MUST 只有一个计数账本，由运行期持有，全部来源共用：设置页展示、切后台、
Surface 销毁、交换链重配防抖、输入法网格重排防抖。运行期的 `setRenderPaused` 是非计数布尔，任何
来源 MUST NOT 直接写它——先结束的来源会把别人还在等的暂停放掉。

两个防抖窗口重叠时 MUST 计数，并 MUST 在替换/取消一个在途防抖时先归还它持有的
那一次——被 `removeCallbacks` 丢弃的 runnable 永不执行，它领到的暂停也就永不归还，
计数单调增长，共享渲染器被**永久**暂停（终端全黑）。键盘弹出动画期间平台逐帧派发
insets，每个动画帧都会走一遍这条替换路径。

**全部来源都经账本**：设置页、切后台、Surface 销毁、视图的两个防抖。
直接写 `setRenderPaused` 会把别人仍在等的暂停顶掉——在已被系统回收的 BufferQueue
上出帧即永久黑屏。任何一个来源 MUST NOT 直写那个非计数布尔；否则它不被任何持有者
计数，别人归还时顺手把它清掉。

一次来源的**替换**（切后台重复发生、换链防抖被新尺寸替换）MUST 先领新的、再归还
旧的——持有者数经过 0 即会多发一对暂停/恢复，而那一对恢复恰好落在最不该出帧的时刻。

取消视图的两个在途防抖 MUST **只归还本视图领到的那两个凭证**：MUST NOT 整本清账
（那会连带作废设置页那条与本视图无关的暂停），也 MUST NOT 只把凭证字段置空——
清字段不等于归还，一次漏还即让计数永不归零，此后所有领用都不再发出暂停（布尔幂等），
暂停功能就此静默失效。

视图拆除的归零入口 MUST 一并归还它领到的那些凭证：`ImeShiftController.reset()` 撤掉
输入法网格重排的防抖 runnable 时，MUST 同时归还该防抖领到的渲染暂停——runnable 被
撤即永不执行，不归还即让计数停在 1，共享渲染器被永久暂停（终端全黑）。归还幂等
（账本按凭证去重），故与拆除路径上先行的归还不冲突。

「刚拿到可用 Surface、必须立刻出一帧」的入口（Surface 重建成功、交换链重配完成）
MUST 只作废**生命周期**持有（Surface 销毁、切后台）；设置页那条 MUST 保留——它是
界面层的独立意图，而终端视图在设置页覆盖层之下仍会收到尺寸变化（设置页自己的
键盘），整本清账会把「进入设置时暂停渲染」废掉。

其中「Surface 销毁」那次持有 MUST NOT 在新窗口挂上原生**之前**被作废：平台回收
Surface 时已废弃它的 BufferQueue，此刻恢复渲染会让那一帧落在已废弃的队列上（取帧
失败，交换链就此不可用）。该次持有 MUST 只由「新窗口挂上原生」解除。

#### Scenario: 关设置页不放开切后台的暂停

- **WHEN** 应用在后台（切后台已持有一次暂停）时用户关闭设置页
- **THEN** 渲染保持暂停，`setRenderPaused(false)` 不被调用

#### Scenario: 取消在途防抖须归还而非只置空凭证

- **WHEN** 防抖窗口内触发强制出帧，视图随即取消在途防抖
- **THEN** 两个在途防抖各自归还其持有的那一次，账本计数归零且设置页的持有不受影响

#### Scenario: 强制出帧只作废生命周期持有

- **WHEN** Surface 重建成功而此前仍有切后台或 Surface 销毁的暂停持有
- **THEN** 这两个生命周期持有被作废，渲染恢复，随后新领的暂停仍能发出；设置页
      仍在展示时则保持暂停

#### Scenario: 设置页暂停先于防抖归还

- **WHEN** 设置页关闭时仍有交换链防抖在途
- **THEN** 渲染保持暂停，直到该防抖也归还

设置页那次暂停的领用 MUST 有确定的归属：凭证一旦领到，要么被状态持有并由拆除路径
归还，要么当场归还；MUST NOT 出现「领了但没有任何路径归还」的持有。设置页可能在
领用协程运行期间关闭——此时拆除路径已跑过且没拿到凭证，故领用 MUST 在**不可取消段
内**、且在领到之前再读一次设置页的**当前**状态（effect 体里捕获的是启动时的快照，
设置页关掉后它仍是旧值）。设置页已关闭时 MUST NOT 再领一次。

领与存 MUST 合成一步（不可取消段内完成「领 + 写入状态」）：段的出口被取消时凭证仍
已落到状态上，由拆除路径归还；否则凭证会落在既没人持有、也没人归还的地方。

#### Scenario: 设置页在领用前关闭不留悬空暂停

- **WHEN** 设置页展示期间关闭，关闭发生在领用协程拿到凭证之前
- **THEN** 账本计数归零，`setRenderPaused(false)` 按其余来源的持有情况决定是否调用；
      不得出现计数停在 1 的悬空持有

视图 detach MUST 取消上述订阅与两个在途防抖、复位平移量与遮挡高度，且 MUST
按发布者归零运行期的 `imeInsetFlow`：订阅跑在 `viewModelScope` 上（跟宿主而非
视图），保留会让旧视图被协程钉住，且 detach 后的备用屏翻转仍会命中网格重排——
此时 `View.postDelayed` 落进 `mRunQueue`、只在重新 attach 时被 drain，被丢弃的
旧视图永不 attach，配对的「恢复渲染」永不执行。只清视图内的字段会让新视图的首次
派发算出 0、与它自己的字段相等而被门控挡下，运行期永久保留旧键盘高度。

#### Scenario: 键盘动画期间渲染不永久停摆

- **WHEN** 备用屏下输入法弹出动画期间平台逐帧派发 insets，防抖被连续替换数十次
- **THEN** 动画结束后渲染恢复；停留期间终端不黑屏（计数归零后 `setRenderPaused(false)`
  必须被调用）

#### Scenario: 换 Surface 后新视图的遮挡高度不被旧视图清掉

- **WHEN** 键盘展开时 Surface 被判死而重建，新 Surface 已发布真实遮挡高度，
      旧 Surface 随后 detach
- **THEN** 运行期仍持有新 Surface 发布的高度，键栏位置与网格扣减量正确

#### Scenario: 主线程被渲染阻塞时位移仍即时

- **WHEN** 每帧绘制都阻塞在等待渲染线程，且输入法在弹出
- **THEN** 终端 Surface 的平移量在 insets 派发的同一拍内生效，不等组合重组

#### Scenario: 旋转后平移量按新视口高度重算

- **WHEN** 输入法已展开时旋转设备（Activity 不重建，同一视图实例尺寸改变）
- **THEN** 平移量按新的视口高度重算，不停留在旧高度的结果上

#### Scenario: 备用屏改字号后仍按可见高度排

- **WHEN** 备用屏下输入法已展开，随后改变字号
- **THEN** 网格仍等于可见高度容纳的行数，不被撑回被键盘遮住的高度

#### Scenario: 备用屏弹出输入法后网格收缩并触发重排

- **WHEN** helix 处于备用屏且输入法弹出并稳定
- **THEN** 网格行数收缩到可见高度能容纳的行数、列数不变，helix 重绘后状态行可见

#### Scenario: 备用屏弹出输入法不隐藏顶部

- **WHEN** helix 处于备用屏且输入法弹出，网格内容填满视口
- **THEN** 终端 Surface 位移为 0，helix 首行仍在屏幕内可见

#### Scenario: 键盘已展开时启动 TUI 也重排

- **WHEN** 主屏 shell 会话中输入法已展开，随后启动 helix
- **THEN** 网格立即收缩到可见高度，helix 按新行数布局

#### Scenario: 键盘保持展开时离开备用屏复原行数

- **WHEN** helix 处于备用屏且输入法仍展开，应用退出到主屏
- **THEN** 网格行数立即回到整屏高度容纳的行数，不停留在被输入法缩小后的值

#### Scenario: 收起输入法后网格复原

- **WHEN** 输入法收起
- **THEN** 网格行数回到弹出前的值

#### Scenario: 换视图后不残留旧键盘高度

- **WHEN** 键盘展开时 Surface 判死导致视图被替换
- **THEN** 新视图接管后键栏与网格都不带旧键盘高度，首次 insets 派发即可自愈

#### Scenario: 备用屏会话关闭后新会话不继承备用屏状态

- **WHEN** helix 处于备用屏且输入法展开时关闭该会话，由另一会话接替
- **THEN** 新会话的网格按整屏高度计算（不扣任何输入法遮挡），键盘弹出时仍随键盘上移

#### Scenario: 选区在键盘弹出后不留陈旧弹窗

- **WHEN** 选中文本后弹出输入法
- **THEN** 选区手柄与上下文菜单被关闭，不留在弹出前的位置

#### Scenario: 主屏位移公式不变

- **WHEN** 终端处于主屏且内容填满网格
- **THEN** 位移仍为整块键盘高度，与备用屏判定无关；`rows`/`cols` 不变，
      上移后底部像素与上移前完全相同

### Requirement: 换 Kitty 图集即失效累加器

替换或清空 Kitty 图像图集 MUST 使下一帧走全量自包含重绘（累加器失效位）。图集内容
变了，而未重绘的行上留的是上一张图的残片；清空时后果更明显——图像消失后实例表为空，
部分帧重新放行，而图像原先压住的那些行若并不脏（kitty 图常叠在空白单元格上），
此后每帧只画脏带，旧图像像素在屏幕上永久残留。

#### Scenario: 删除图像后不残留像素

- **WHEN** 某张 kitty 图像覆盖的行本身并不脏，图像随后被删除
- **THEN** 删除后的下一帧为全量重绘，旧图像像素不再留在屏幕上

### Requirement: surface 挂载失败必须让出属主并判死

任一挂载失败路径（`ANativeWindow_fromSurface` 返回空、`ANativeWindow_setBuffersGeometry`
非零、`create_surface_unsafe` / `get_capabilities` / `configure` 失败）MUST 同时做两
件事：把当前 surface 属主清零，并把 surface 标记为已判死。缺任一条的后果相同且不可
自愈：属主残留时它随后的 `detachWindow` 仍能通过归属校验并清掉状态，而新窗口无人挂载；
而渲染对「无 surface」返回 0，与空闲帧不可区分，宿主的换窗口看门狗因此永远不触发。

#### Scenario: 几何设置失败也走同一条自愈路径

- **WHEN** 挂载时交换链几何设置失败
- **THEN** 属主被清零且 surface 判死上报，宿主据此换新窗口重新挂载

### Requirement: 重建与释放前必须排干在途取帧

取纹理工作线程在在途取帧期间持有 surface 的克隆。任何丢弃或重建 wgpu surface 的路径
（慢路径换窗、`release_surface`）MUST 先排干在途取帧（等待克隆结清）之后才允许重建
或释放；同一 `ANativeWindow` 地址上 MUST NOT 并存两个 wgpu surface 实例。该契约由
判定表单测直接覆盖，改动判定逻辑 MUST 保持其判红。

排干失败（超时上限 2 秒）且目标恰好是仍带在途克隆的窗口时，MUST 拒绝该次挂载：错误
明文上抛为 `GpuError`，同时让出属主并上报判死，走「surface 挂载失败必须让出属主并
判死」的同一条自愈路径；MUST NOT 任由底层 panic 跨 JNI 出现。排干超时只作用于重建
与释放点；每帧挂载前的快路径重配等待的是毫秒级正常排干，MUST NOT 为快路径支付
超时成本。

#### Scenario: 同址重建前先排干在途克隆

- **WHEN** 重建请求到来，而该窗口仍有在途取帧的 surface 克隆
- **THEN** 先等待克隆结清（排干）完成，之后才创建新 surface，panic 不发生

#### Scenario: 排干失败时拒绝同窗重建并判死自愈

- **WHEN** 排干超时（上限 2 秒），且目标恰是仍带在途克隆的那一个窗口
- **THEN** 该次挂载被拒绝、错误明文上抛，属主清零并判死，宿主由此换新窗口重挂

#### Scenario: 换入新窗口不因在途克隆被误伤

- **WHEN** 排干超时，但目标窗口没有在途克隆记录
- **THEN** 该窗口的挂载继续执行，重建请求不被拒绝

### Requirement: Surface 尺寸钳制到设备上限

交换链的可配置尺寸 MUST 由唯一一个纯函数计算：先按跨渲染器固定的
`RENDER_SCALE` 缩放，再钳制到 `[1, device.limits().max_texture_dimension_2d]`。
`attach_surface` 与 `reconfigure_swapchain` 两条挂载路径 MUST 共用该函数，
MUST NOT 各写一份缩放/钳制逻辑——补了钳制的那条路径会给另一条留下缺口，而
该缺口在 wgpu 30 无返回值、只经 `get_current_texture` 的 Lost 状态异步上报，
表现为永久黑屏且没有直接错误。

钳制 MUST 在宿主单测里以判定表直接判定（不经 GPU）：缩放、下限 1、上限钳制
三个不变量各有用例；改动钳制逻辑 MUST 让对应用例判红。

#### Scenario: 超限窗口尺寸被钳制而非交给 configure 失败

- **WHEN** 窗口缩放后尺寸超过 `max_texture_dimension_2d`
- **THEN** 配置尺寸等于该上限，surface 正常出帧，MUST NOT 以超限尺寸 configure

#### Scenario: 两条挂载路径共用同一计算

- **WHEN** 复查 `attach_surface` 与 `reconfigure_swapchain` 的尺寸来源
- **THEN** 两者都调用同一纯函数，MUST NOT 出现第二份缩放/钳制实现

#### Scenario: 下限为一像素

- **WHEN** 传入尺寸为 0（空窗口／尚未布局）
- **THEN** 结果两维都不小于 1，与 wgpu 对零尺寸 surface 的要求一致

### Requirement: 开孔无缓冲区时由 SurfaceView 自己盖住

`SurfaceView` 的原生 surface 位于应用窗口**之下**，窗口在该区域是透明开孔：缓冲区
一旦缺失（冷启动首帧未出、切后台回来重建窗口、原生判死后换视图），开孔就透出后方
的桌面或上一个应用，表现为「切换应用返回一闪」。

窗口层 MUST NOT 用来遮这个洞：平台在 `SurfaceView` 绘制时给窗口打洞，洞里窗口层
画的像素一律被抹掉。

宿主 MUST 在「当前 Surface 尚无缓冲区」期间，由 `SurfaceView` 在 `onDraw` 里于开孔
区域绘制离开前最后一帧的遮罩（见「运行期缓存离开时的画面作为开孔遮罩」）；没有遮罩
缓存时退回与当前终端主题背景一致的颜色，并在该 Surface 首次成功提交缓冲后立即撤销。
撤销 MUST 以「已提交」为条件，MUST NOT 以时间或重试次数为条件。

宿主 MUST 区分「换了 Surface 实例」与「同一 Surface 改尺寸」：只有前者使开孔失去
缓冲区，旋转、输入法重排 MUST NOT 触发这次覆盖。

#### Scenario: 冷启动首帧前不黑洞

- **WHEN** 应用冷启动，原生尚未提交第一帧
- **THEN** 开孔区域显示终端主题背景色，MUST NOT 显示黑色或桌面

#### Scenario: 切后台回来不闪桌面

- **WHEN** 应用从后台返回，系统交付了新的 `Surface` 实例且尚无新帧
- **THEN** 开孔区域显示遮罩帧（无缓存时为主题背景色），桌面 MUST NOT 可见

#### Scenario: 首帧提交后撤销

- **WHEN** 该窗口已把遮罩画进开孔，且原生侧已提交缓冲
- **THEN** 覆盖 MUST 在紧接着的一帧撤下（`post` 到下一轮 UI 循环清空遮罩并触发重绘），
      MUST NOT 在 `onDraw` 内直接 `invalidate` 等第二次绘制（实测那次不产生第二次绘制，
      遮罩会一直留在开孔上）；终端内容随即完整可见
- **AND** 撤销 MUST 以「已提交」为条件：提前撤销即退回原缺陷，延后撤销则遮挡画面

#### Scenario: 同 Surface 改尺寸不覆盖

- **WHEN** 旋转或输入法重排导致同一 `Surface` 实例以新尺寸重新挂载
- **THEN** MUST NOT 置「无缓冲区」，画面不闪底色

### Requirement: 开孔覆盖按「本窗口是否画过」与「原生是否有缓冲」两个事实决定

`onDraw` 的内容盖在原生 surface 之上，故遮罩 MUST 只在「开孔里的像素还不是当前
窗口画出来的」时画：之后每一帧都要让位给原生图层，否则遮罩会把终端内容一起盖住。

两个事实 MUST 分开取，MUST NOT 合并成「原生有没有缓冲」一条：

- **原生已提交缓冲**（`TerminalRuntime.surfaceBufferPresent`）只说明渲染侧出过帧，
  MUST NOT 据此认为**当前窗口**已经画过遮罩：平台可能刚把窗口重挂上来，旧窗口的
  缓冲连同遮罩一起丢了。
- **当前窗口画过这份遮罩**（视图按遮罩对象身份记账）才是「洞里已有内容」的证据。

故判据是：遮罩在位时，后台（`isBackgroundPaused`）照画——转场里平台还没拆
surface，遮罩必须这时进窗口缓冲；前台只在**本窗口还没画过这份遮罩**时补画一帧。
撤销 MUST 以「原生已提交 + 本窗口已画过」为条件，MUST NOT 以时间或重试次数为条件。

#### Scenario: 有缓冲且本窗口已画过遮罩

- **WHEN** 原生 surface 已提交过缓冲，且当前窗口已画过这份遮罩
- **THEN** `onDraw` MUST NOT 画覆盖，终端内容完整可见

#### Scenario: 窗口重挂后的首个可见帧

- **WHEN** 平台把窗口重挂（旧缓冲连同遮罩丢失），而原生侧已提交过缓冲
- **THEN** 首个可见窗口帧 MUST 补画遮罩，MUST NOT 留下空开孔
- **AND** 去掉「本窗口已画过」这道闸后，开孔里只剩一块平底色

#### Scenario: 判据取自运行期的事实

- **WHEN** 视图判断是否画覆盖
- **THEN** 「原生是否已提交缓冲」MUST 取自运行期记录的事实，MUST NOT 由视图猜测

### Requirement: 增量渲染与全量渲染输出一致

脏带（部分）路径只决定「哪些行重画」，MUST NOT 改变行内容：同一份单元数据与光标
下，增量构建的输出 MUST 与全量构建逐字节一致。判据落在实例输出上——CPU 层一致后，
GPU 侧只多出「画哪些行」，其像素正确性由滚动残留与脏带平铺范围用例分别覆盖。

#### Scenario: 只标一行脏的帧等于全量帧

- **WHEN** 先以全脏掩码填充行缓存，再对同一份单元数据只标记一行脏
- **THEN** 增量输出与全量输出逐字节相同（用例 `cached_build_matches_full_build_byte_for_byte`）

#### Scenario: 光标行参与增量

- **WHEN** 光标所在行被标脏
- **THEN** 该行连同光标四边形一并重建，输出仍与全量一致

### Requirement: 增量的 UV 必须按当前图集解释

干净行复用的是**上一帧构建好的实例**，实例的 UV 只在图集搬迁后失效，而搬迁随时可能发生
（本帧脏行把干净行的字形挤出图集、抢占式预习的第一遍本身也会搬迁）。故单元构建 MUST
先预热本帧**要重建的行**里的全部字形、再在代际不再推进的前提下发射，并且 MUST 只把
「全程未发生搬迁的那一遍」写回实例缓存；任一趟发射推进了代际时 MUST 降级为全量重建，
MUST NOT 把搬前的 UV 连同搬后的代际一起存入缓存——下一帧的增量判定会认这个代际为
「未变」，干净行照着已被别人占用的区域采样，同一个字符在屏幕上画出两种字形。

反之，若工作集本身大过图集容量，则遍数再多也不可能达成「不搬迁的一遍」（每次查询都在挤
走另一个字形）。此时 MUST 出声而不 MUST NOT 静默无限重发射：实例表当场可用但 WHERE 属性
无法自证一致，屏幕上的字会在字符之间 variant，而唯一的归因线索就是这条告警。告警 MUST
按 logcat-hygiene 限频，且 MUST NOT 把那种帧写入实例缓存。

#### Scenario: 帧内搬迁后 UV 仍按当前图集解释

- **WHEN** 一帧里干净行沿用旧字形、脏行引入的新字形把前者挤出图集
- **THEN** 该帧每个实例的 UV MUST 等于当时字形缓存给出的位置，
      MUST NOT 沿用搬迁前的旧坐标（用例 `incremental_frame_with_midframe_eviction_has_current_uvs`）

#### Scenario: 写回缓存的那一趟没有搬迁

- **WHEN** 某一趟发射过程中图集代际发生了变化
- **THEN** MUST 全量重发一遍，且只有末趟代际未变时才写回实例缓存；
      一路不收敛时 MUST NOT 留下任何缓存条目

#### Scenario: 工作集超过图集容量时告警

- **WHEN** 一帧的去重字形数在图集里放不下，任意一遍发射都会挤走另一个字形
- **THEN** MUST 限频记一条 warn 说明图集在抖动，且 MUST NOT 写回实例缓存；
      MUST NOT 无限重发射——那是停顿而非修复

### Requirement: 预热遍只覆盖要重建的行并按字形去重

增量帧的预热范围 MUST 只覆盖脏行：干净行的字形早已在图集里，而缓存代际与当前代际一致
即证明它们没被驱逐、UV 也没搬迁，整屏预热是白扫。预热若把干净行的字形挤出图集
（代际推进），MUST 降级为全量重建并整帧预热：全量重建要整帧字形都在图集里，否则发射遍
边建边光栅化会再次驱逐。

预热遍 MUST 按字形去重：同一帧里重复出现的字形（码位 + 粗斜样式）只光栅化一次。一屏
1920 单元通常只含几十种字形，逐格光栅化只是把同一次查表做上几百遍。

#### Scenario: 增量帧只预热脏行

- **WHEN** 增量帧只有一行是脏的
- **THEN** 预热遍 MUST 只访问该行的单元，MUST NOT 扫全屏（用例
      `incremental_frame_warms_only_dirty_rows`）

#### Scenario: 预热引发驱逐时降级为全量重建

- **WHEN** 脏行预热把干净行的字形挤出图集（图集代际推进）
- **THEN** MUST 降级为全量重建并整帧预热（用例
      `incremental_frame_falls_back_to_full_rebuild_when_warm_evicts`），该帧每个实例的 UV
      MUST 等于当前字形缓存给出的位置（用例
      `incremental_frame_uv_matches_atlas_after_eviction`）

#### Scenario: 同一字形在一帧里只请求一次

- **WHEN** 一帧里同一个字形出现几十次
- **THEN** 预热遍 MUST 只向字体管线发一次该字形的请求，MUST NOT 逐格重复请求
      （用例 `warm_frame_deduplicates_repeated_glyphs`）

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
- **THEN** 测试通过，且入口点集合恰为 `vs_main` 与 `fs_main`

实现细节：

- 着色器源码为 `render/pipeline.rs` 的 `CELL_SHADER` 与 `KGP_SHADER` 两个常量，
  由 `include_str!` 编译期内联；管线创建与校验测试共用，不存在第二处内联。
- 校验经 `wgpu::naga`（`wgpu` 无条件再导出 `wgpu::naga`）完成，无需新增依赖，也无需
  GPU 适配器，因此在无 lavapipe 的环境下同样执行而非跳过。
- 校验分两步且分别断言：先 `wgsl::parse_str` 解析，再以
  `Validator::new(ValidationFlags::all(), Capabilities::default())` 校验 IR；两步失败
  均以 `emit_to_string` 的 naga 诊断作为断言信息。

### Requirement: 帧交付必须保最新帧

VT 线程把单元帧交给渲染线程时，MUST 保留最新一帧：后到的帧覆盖先到的帧，
MUST NOT 因「来不及取走」而丢弃新帧。消费者取帧时 MUST 取到当前最新画面。

帧是网格全量快照，中间帧没有呈现价值；渲染暂停期（输入法弹出、进入设置）更是
刻意不取帧——被丢的新帧没有任何补发途径（内容去重基线在发布时已推进），
屏幕将停在旧画面。

#### Scenario: 突发推帧后取到的是最新画面

- **WHEN** VT 线程连续推送多帧而消费者始终不取
- **THEN** 消费者取到的 MUST 是最后一帧，MUST NOT 是若干帧之前的画面

#### Scenario: 暂停期输出后恢复取到的是最新画面

- **WHEN** 渲染暂停期间终端持续输出，恢复后取帧
- **THEN** 取到的 MUST 是暂停期结束时的最新画面

### Requirement: 渲染恢复 MUST 晚于新窗口挂载

平台回收 Surface 时会废弃它的 BufferQueue；此后渲染线程再取帧只会得到
`dequeueBuffer failed: No such device (-19)` 与 `ERROR_SURFACE_LOST_KHR`，交换链就此
不可用（返回后永久黑屏）。故「Surface 销毁」那次渲染持有 MUST 只由**新窗口挂上原生**
解除；恢复渲染的路径（切前台、延迟解除暂停、强制出帧）MUST NOT 解除它。

新窗口挂上 MUST 先于解除持有：恢复渲染的入口 MUST 先判「新窗口是否已挂上」，未挂上时
MUST 保持暂停且 MUST NOT 重启渲染线程（逐帧「render paused」诊断同样是噪音）。

实测形态：切后台后回前台，恢复渲染比新窗口挂载早 27ms，那一帧正落在已废弃的队列上，
报错后靠重建交换链自愈——但那一帧的报错与随后的交换链重建都是可避免的。

#### Scenario: 恢复早于新窗口时不得出帧

- **WHEN** Surface 已销毁且新窗口尚未挂上原生，而恢复路径被触发
- **THEN** 渲染 MUST 保持暂停，MUST NOT 在已废弃的 BufferQueue 上取帧

#### Scenario: 新窗口挂上后解除该次持有

- **WHEN** 新窗口挂上原生
- **THEN** 「Surface 销毁」那次持有 MUST 被解除；此后新领的暂停 MUST 仍能发出

#### Scenario: 重复销毁不得让持有者数经过 0

- **WHEN** 新窗口迟迟不来而 Surface 被再次销毁
- **THEN** 持有者计数 MUST 全程非空，MUST NOT 多出一对暂停/恢复

### Requirement: Surface 归属认领 MUST 先于任何提前返回

视图 MUST 在 `surfaceCreated` 一进入就认领 Surface 归属；0 尺寸、无效 Surface
等提前返回 MUST 发生在认领之后。认领的语义是「本视图自此是唯一有权暂停
渲染的那个」，与尺寸无关。

认领被跳过时的故障形态不可自愈：旧视图迟到的 `surfaceDestroyed` 仍通过归属
校验，清空待绑定的 Surface 并领走一次渲染暂停；新窗口的绑定随后已经发生，
那次暂停无人归还，终端停在空白画面，全程零异常。

#### Scenario: 零尺寸开孔也要认领

- **WHEN** `surfaceCreated` 收到的宽高为 0
- **THEN** 本视图 MUST 已认领归属；旧视图迟到的销毁 MUST 被识别为陈旧

#### Scenario: 无效 Surface 开孔也要认领

- **WHEN** `surfaceCreated` 拿到的 Surface 已失效
- **THEN** 本视图 MUST 已认领归属，且 MUST NOT 绑定该 Surface

### Requirement: 持续输出时帧不得停发

VT 线程 MUST 在持续输出下持续发布帧：命令批有界（见 `vt-query-latency`），批尾即帧
发布点，批按上界反复切分使帧不被无限期推迟。剩余输出 MUST 可见。

帧发布 MUST NOT 因「上一帧尚未被渲染线程取走」而跳过构建：帧是全量快照，后到的帧覆盖
先到的帧，跳过构建会让最新状态滞留到下一批，消费者先取到的仍是旧帧，与「帧交付必须
保最新帧」相悖。抽取成本 MUST 由「每批至多构建一帧」约束，而不是由丢弃最新帧约束。

实现：`native/src/terminal/ghostty_terminal/internal/run.rs` 的批处理循环受
`MAX_COMMANDS_PER_BATCH` 与 `COMMAND_BATCH_BUDGET` 约束；`publish_cell_data` 不检查
邮箱是否已有未被取走的帧。

#### Scenario: 输出较多时剩余内容可见

- **WHEN** 终端持续输出，输出量超过一帧能显示的容量
- **THEN** 屏幕 MUST 持续推进到最新内容，MUST NOT 停在某一帧不再更新

#### Scenario: 上一帧未被取走时最新帧仍会送达

- **WHEN** 渲染线程迟迟没有取走上一帧，终端又产生了新内容
- **THEN** 渲染线程下一次取到的 MUST 是最新一帧，MUST NOT 是若干帧之前的画面

### Requirement: 交换链格式必须是能承载颜色的格式

交换链格式 MUST 至少有三个颜色分量。应用 MUST 从适配器报告的表面格式里**指名**取
8 位 RGBA 族（`Rgba8Unorm` 优先），退化时也只能退到三通道以上的格式；单通道与双通道
格式 MUST NOT 被选中——它们渲染不出颜色，且 R8（`AHARDWAREBUFFER_FORMAT_R8_UNORM`，
0x38）在部分设备的分配器上根本申请不到。设备只报告这类格式时 MUST 报错，MUST NOT
带着不可分配的格式去配置交换链。

#### Scenario: 设备把 R8 排在首位

- **WHEN** 适配器报告的表面格式以 `R8Unorm` 开头且同时含 `Rgba8Unorm`
- **THEN** 交换链格式 MUST 取 `Rgba8Unorm`，MUST NOT 取 `R8Unorm`

#### Scenario: 只剩单通道格式

- **WHEN** 适配器只报告单通道或双通道格式
- **THEN** 配置 MUST 报错退出，MUST NOT 选中该格式去申请窗口缓冲

### Requirement: 管线建立不得发生在渲染循环内

cell 与 KGP 两条文本管线的着色器编译与图集分配 MUST NOT 在渲染循环内惰性执行：
这两步是毫秒到百毫秒级的一次性代价，放在每帧的兜底路径上会把首帧与重建帧一起
拖住。

两条文本管线 MUST 都由挂载路径（窗口挂载成功分支）主动完成，MUST NOT 占住 UI
线程；渲染路径 MAY 保留兜底补建（挂载路径未走到时仍要能出帧），但该路径 MUST
记录建立耗时（含 KGP 管线），代价可见。

#### Scenario: 挂载后首帧不再等待管线建立

- **WHEN** 窗口挂载成功
- **THEN** cell 与 KGP 两条管线在挂载路径上建立完毕，渲染循环的首帧 MUST NOT
      再承担任一管线的建立代价

#### Scenario: 首幅 kitty 图像不承担管线编译

- **WHEN** 终端首次呈现 kitty 图像
- **THEN** KGP 管线 MUST 已在挂载路径建立；该帧的 `SLOW_RENDER_INNER` 若仍超
      阈值，MUST 只归因于图像查询与上传，MUST NOT 包含管线编译

#### Scenario: 兜底补建留下可观测记录

- **WHEN** 渲染路径确实需要补建 cell 或 KGP 管线
- **THEN** 日志 MUST 记录该次建立的耗时（KGP 为
      `ensure_kgp_pipeline: created pipeline ... elapsed`）

### Requirement: 图集重传只覆盖缓存字形

管线重建后 GPU 侧的图集纹理是新建的空纹理，而 CPU 侧字形缓存仍全命中——没有任
何字形被重新 blit，终端只剩背景、无文字且不自愈。故重建 MUST 把缓存字形重新
上传。

重传 MUST 只覆盖缓存字形的包围盒，MUST NOT 整幅标脏：整幅图集在 2048×2048 下
是 16MB，而实际用到的字形通常只有几十 KB。

#### Scenario: 重建后文字重新出现

- **WHEN** 管线重建后 CPU 侧字形缓存全命中
- **THEN** 缓存字形 MUST 被重新上传，终端 MUST NOT 只剩背景无文字

#### Scenario: 重传范围不含图集空白

- **WHEN** 管线重建后重传图集
- **THEN** 脏区 MUST 是缓存字形的包围盒，MUST NOT 是整幅图集

### Requirement: 放不进图集的字形不得驱逐缓存或重建图集

单个字形比整张空图集还大时它永远放不进去（分配的是字形尺寸加一像素间隙）。此时
驱逐缓存与重建图集都救不了它：重建只会把同样放不下的字形再放一次。故 MUST 在驱逐
与重建之前判定并放弃该字形，MUST NOT 触发驱逐或重建。

该放弃 MUST 逐帧限频告警并带上窗口内累计次数：逐帧输出会把日志刷成洪水。

#### Scenario: 放不下的字形不触发驱逐与重建

- **WHEN** 单个字形的尺寸加一像素间隙已超过图集边长
- **THEN** 该字形 MUST NOT 被分配，MUST NOT 驱逐缓存字形，MUST NOT 重建图集
      （用例 `oversized_glyph_neither_evicts_nor_rebuilds_the_atlas`）

#### Scenario: 放弃字形的告警按窗口限频

- **WHEN** 同一帧内有多个放不下的字形被放弃
- **THEN** 告警 MUST 按限频窗口输出，并 MUST 带上窗口内的累计次数

### Requirement: 字形缓存条目独占其图集分配

字形缓存里的每条 MUST 恰好占一块图集分配，且图集里 MUST NOT 存在无主的分配。
LRU 缓存写满时库自行淘汰最久未用的条目，那条目的分配 MUST 随之归还：分配随
`GlyphInfo` 析构并不等于空间被回收，不归还就会在分配器里留下永远占着的死区，
图集被死区吃满后每来一个新字形都要重建整张图集（重光栅化全部缓存字形），
终端表现为「越用越慢」，而根因是早已无人引用的分配。

归还会改写该区域的像素，故 MUST 同时推进图集代际：仍引用旧 UV 的实例缓存否则
会采样到别人的像素。

#### Scenario: 淘汰条目不留死区

- **WHEN** 字形缓存写满并淘汰最久未用的条目
- **THEN** 该条目占用的图集分配 MUST 被归还，图集里的活分配数 MUST 等于缓存条目数
      （用例 `glyph_cache_eviction_reclaims_the_atlas_allocation`）

### Requirement: 部分帧只上传脏带实例

部分帧（脏带路径）MUST 只把脏带覆盖的实例切片上传到实例缓冲：字节偏移 MUST 与
绘制时使用的槽位序号同坐标系（清除段末尾 + 实例序号 × 步长），MUST NOT 上传
干净行的实例。全量帧 MUST 继续上传全部实例。

上传范围 MUST 覆盖本帧绘制的全部槽位：漏传的槽位会让 GPU 读到上一帧的实例
（画面错字或错位），MUST NOT 发生。

#### Scenario: 脏带切片按槽位偏移上传

- **WHEN** 部分帧有两个脏带，分别覆盖第 2–3 与第 10–12 个实例
- **THEN** 上传 MUST 只有两段，偏移 MUST 分别落在清除段末尾 + 2 × 步长与
      清除段末尾 + 10 × 步长（用例 `partial_instance_uploads_cover_only_dirty_bands`）

#### Scenario: 空切片与越界切片不产生非法上传

- **WHEN** 脏带的实例切片为空或整段落在本帧实例列表之外
- **THEN** 上传 MUST NOT 产生越界区间（用例 `partial_instance_uploads_skip_empty_and_out_of_range_bands`）

#### Scenario: 全量帧仍上传全部实例

- **WHEN** 本帧走全量重绘路径
- **THEN** 实例缓冲 MUST 收到整份实例，不按脏带裁剪

### Requirement: 交换链格式必须与窗口缓冲格式一致

宿主挂载 Android 窗口时 MUST 以窗口缓冲格式 `RGBA_8888`
（`WINDOW_FORMAT_RGBA_8888` = `AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM`）设置缓冲
几何；交换链 MUST 选用与之同格式的表面格式（Vulkan 上的
`VK_FORMAT_R8G8B8A8_UNORM`，即 wgpu 的 `Rgba8Unorm`）。两处 MUST 取自同一处定义，
MUST NOT 各写一份字面量。

两处不一致时，缓冲队列按另一种格式分配缓冲，呈现出来的像素布局与渲染写入的布局
不一致；单通道格式即便能呈现，也只有灰度一个通道。

单通道与双通道格式 MUST NOT 被选为交换链格式：颜色渲染不出来。

#### Scenario: 窗口缓冲格式与交换链格式同源

- **WHEN** 宿主挂载窗口并创建交换链
- **THEN** 窗口缓冲格式 MUST 为 `RGBA_8888`，交换链 MUST 为 `Rgba8Unorm`，
      两者 MUST 取自同一处定义

#### Scenario: 不得选用单通道交换链格式

- **WHEN** 适配器报告的候选格式里排在前面的单通道格式
- **THEN** 交换链 MUST NOT 选用它，MUST 退到多通道格式；退化 MUST 记一条日志

#### Scenario: 申请格式必须可被分配器分配

- **WHEN** 应用申请交换链缓冲格式
- **THEN** 该格式 MUST 被设备分配器支持（`Rgba8Unorm` / `RGBA_8888` 是各设备通用的
      颜色格式）；平台在格式枚举阶段自行发起的探测不在此列，MUST NOT 据此改动应用
      请求的格式
- **AND** 日志里分配器拒绝某格式的行（如 `mali_gralloc` 拒绝 R8）MUST 归因于平台
      `libvulkan` 枚举表面格式时的固定探测（判据见 `logcat-hygiene` 的「设备分配器的
      格式探测告警」），MUST NOT 据此改动应用请求的格式

### Requirement: 终端 Surface 的表面策略跟随平台默认

终端 SurfaceView MUST 使用平台默认的表面生命周期策略，MUST NOT 以 `setSurfaceLifecycle`
改写它。

`SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT`（API 34+）只让 surface 跟随**视图的挂载**，
不改变平台在后台拆掉本应用**窗口** surface 这一事实：实测（API 35 模拟器）切后台时
`surfaceDestroyed` 仍会到达，返回前台时原生窗口指针已变、交换链照样重建。改写该策略
既没有消除重建，也把「平台是否会替我们保留 surface」这种不可控的假设写进了代码。

每次重建都会让驱动重新枚举表面格式并试分配，真机每次留下 4 组（共 8 条）设备图形
分配器的格式探测 error 与一串平台帧时间线的同步组查询 error——这类条目是重建次数的
函数（见 `logcat-hygiene`），重建本身不可免，能消除的只是用户可见的闪烁。

#### Scenario: 不改写表面生命周期策略

- **WHEN** 终端 SurfaceView 初始化
- **THEN** MUST NOT 调用 `setSurfaceLifecycle`，MUST 保持平台默认策略

#### Scenario: 视图脱离窗口时 surface 仍被拆除

- **WHEN** 终端 SurfaceView 脱离窗口（Activity 销毁或宿主换视图）
- **THEN** surface MUST 按平台回调拆除，GPU 表面 MUST 随之释放

### Requirement: 新 surface 挂上时先呈现保留的上一帧

原生侧在新 surface 挂上、渲染循环的首帧尚未产出时 MUST 先呈现帧累加器里保留的
上一帧，MUST NOT 直接清成背景色：那一帧正是用户离开时看到的画面，直接清屏就是
「切应用返回闪一下」。

判据 MUST 是「累加器里有一帧**已渲染**的内容」，MUST NOT 是「内容还新鲜」：渲染
暂停期写入的输出让累加器变陈旧，但那正是用户离开时看到的那一帧，要呈现的正是它。
内容新鲜度管的是**下一帧**能否走部分渲染路径，与此处无关。

累加器从未渲染过（内容未定义）、或其尺寸与格式与新交换链不一致时 MUST 退回清成
背景色，MUST NOT 呈现未初始化或错位的纹理。

#### Scenario: 返回前台时呈现离开时的画面

- **WHEN** 应用从后台返回，新 surface 挂上且渲染循环尚未产出首帧
- **THEN** 原生 MUST 呈现累加器里保留的上一帧，画面与离开时一致

#### Scenario: 暂停期写入过输出仍呈现保留帧

- **WHEN** 渲染暂停期间有输出写入，累加器因此变陈旧
- **THEN** 新 surface 挂上时 MUST 仍然呈现该累加器内容，MUST NOT 因「陈旧」而清屏

#### Scenario: 从未渲染过时不呈现未定义纹理

- **WHEN** 累加器尚未渲染过任何一帧（冷启动首次挂载）
- **THEN** 原生 MUST 清成背景色，MUST NOT 呈现未初始化纹理

#### Scenario: 尺寸或格式变了时退回背景色

- **WHEN** 保留帧的尺寸或格式与新交换链不一致
- **THEN** 原生 MUST 清成背景色，MUST NOT 呈现错位的内容

### Requirement: 取到的交换链纹理必须呈现归还

渲染 MUST NOT 丢弃已取到但本帧不用的交换链纹理。wgpu-hal 的 Vulkan 后端对「未呈现即被
丢弃」的纹理，`discard_texture` 是空实现：那张交换链图像既不呈现、也不归还，累积几次后
交换链再也拿不到可用图像，终端永久停帧且无自愈。此类纹理 MUST 先清成背景色再呈现
（未清内容可能是未定义像素），呈现 MUST 发生在交换链销毁之前。

- 纹理尺寸与当前配置不符时：MUST 在 `surface.configure` **之前**归还，configure 会销毁
  旧交换链，之后呈现即是悬垂图像；
- 渲染管线缺失、本帧无法绘制时：MUST 归还后再返回错误，MUST NOT 让纹理随帧上下文丢弃；
- 取纹理结果晚于取纹理期限时：MUST 按「迟到结果的归还责任唯一」一条处理。

归还 MUST NOT 被实现为「先记账、下一帧再处理」：纹理在下一帧之前一直占用交换链图像。

#### Scenario: 纹理尺寸与配置不符

- **WHEN** 取得的交换链纹理尺寸与当前 surface 配置不一致
- **THEN** 该纹理 MUST 清成背景色并呈现，随后 MUST 按纹理实际尺寸 reconfigure；
      画面 MAY 显示一帧背景色，MUST NOT 永久停帧，MUST NOT 丢弃该纹理

#### Scenario: 管线缺失

- **WHEN** 本帧已取得纹理，而渲染管线不可用
- **THEN** 纹理 MUST 先归还再返回错误，错误仍按既有口径上报，MUST NOT 静默丢弃纹理

#### Scenario: 归还的纹理被清成背景色

- **WHEN** 归还一张已取得但未绘制的纹理
- **THEN** 该纹理的像素 MUST 等于当前背景色（回归测试
      `returned_texture_is_cleared_to_the_background` 以 GPU 回读断言）

### Requirement: 迟到结果的归还责任唯一

取纹理结果晚于渲染线程的取纹理期限时，MUST 恰好有一方归还该纹理。渲染线程的「放弃」
与工作线程的「回送」MUST 互斥：渲染线程先放弃则工作线程自行归还，工作线程先回送则
渲染线程取回结果并自行归还。两边 MUST NOT 把纹理留在无人接收的通道里——通道析构即
丢弃未呈现纹理，与直接丢弃同一条不可逆路径。

在途记账 MUST 覆盖「纹理尚未归还」的窗口，使释放 surface 前的排干能保证交换链销毁前
纹理已呈现。

#### Scenario: 取纹理超时后的迟到结果

- **WHEN** 渲染线程按 100ms 期限放弃某帧，工作线程此后才取到纹理
- **THEN** 工作线程 MUST 把该纹理清成背景色并呈现，MUST NOT 随发送失败一并丢弃；
      此后的取纹理 MUST 能重新拿到可用图像

#### Scenario: 放弃与回送互斥

- **WHEN** 渲染线程已按期限放弃本帧，而工作线程此后才把取纹理结果回送
- **THEN** 该结果 MUST 恰好被一方取走并归还：回送先发生时渲染线程 MUST 取回并归还，
      放弃先发生时工作线程 MUST 自行归还；MUST NOT 出现「结果躺在通道里、随通道析构
      而被丢弃」的情形

#### Scenario: 释放 surface 前的排干覆盖迟到结果

- **WHEN** 释放或重建 surface 前执行在途取帧排干
- **THEN** 排干返回时 MUST 保证该 surface 的纹理已被呈现或已被渲染线程接收，
      MUST NOT 遗留未归还的纹理导致交换链销毁后再呈现

### Requirement: 渲染暂停期间的会话推进由定时泵接管

渲染线程同时承担「渲染」与「推进会话」两件事。渲染暂停（切后台、打开设置页等）即停掉
渲染线程，推进随之消失：无人消费时有界输出通道会填满 → PTY 读取线程阻塞 → 内核缓冲
填满 → 子进程写入阻塞，后台作业停摆。故暂停期间 MUST 由宿主侧的定时泵接管推进：
会话状态推进、OSC 52 读取请求收割与事件产生 MUST 继续进行。

定时泵 MUST 满足：

- 推进 MUST 是拉取式的（有输出才做事），MUST NOT 以阻塞等待输出实现；节拍 MUST 固定
  有界。
- 渲染恢复时 MUST 停止泵：两个消费者会争抢单一全局事件队列，退出事件可能被错误的
  会话处理。
- 无会话时 MUST 自停，MUST NOT 留下常驻线程。
- 单次推进失败 MUST NOT 终止泵：下一拍自然收敛。
- 推进 MUST NOT 消费事件队列：暂停期间产生的事件 MUST 留在原生队列，由恢复后的取
  事件路径派发。

每拍处理量 MUST 有上限且分会话：活跃会话每拍最多 10 块输出，后台会话每拍最多 2 块；
全量处理会饿死活跃会话的帧预算。

#### Scenario: 渲染暂停期间后台作业继续推进

- **WHEN** 渲染暂停后远端程序持续输出
- **THEN** 会话状态 MUST 继续推进，PTY 输出通道 MUST NOT 填满到让子进程写入阻塞

#### Scenario: 渲染恢复后不留第二个推进者

- **WHEN** 渲染恢复
- **THEN** 定时泵 MUST 已停止，推进 MUST 只由渲染线程承担

#### Scenario: 无会话时泵自停

- **WHEN** 推进泵的一拍到来时没有活跃会话
- **THEN** 泵 MUST 自行停止，MUST NOT 留下常驻线程

#### Scenario: 单次推进失败不终止泵

- **WHEN** 某一拍的推进抛出异常
- **THEN** 该次失败 MUST 记一条告警，泵 MUST 在下一拍继续

#### Scenario: 暂停期间的事件不丢失

- **WHEN** 暂停期间会话产生剪贴板、退出或桌面通知事件
- **THEN** 事件 MUST 留在原生事件队列里，恢复后照常派发

### Requirement: 实例缓存 MUST 是本帧实例的唯一副本

渲染器 MUST NOT 为同一帧的单元实例保留第二份副本：行级实例缓存既是跨帧复用的增量
基线，也是本帧提交给 GPU 的那份实例。MUST NOT 每帧把整屏实例从缓存复制进另一块缓冲
再复制回来——一屏约 100KB，60fps 下即约 6MB/s 的纯搬运，且这份搬运不产生任何画面变化。

干净行 MUST 原地保留在缓存里；脏行 MUST 就地重建并替换，MUST NOT 先把整屏拷走再回填。

#### Scenario: 渲染器不持有第二份实例缓冲

- **WHEN** 检视 `Renderer` 的字段
- **THEN** MUST NOT 存在与实例缓存并列的实例缓冲字段

#### Scenario: 增量帧不产生整屏实例拷贝

- **WHEN** 一帧只有一行变脏
- **THEN** 干净行的实例 MUST 留在缓存原位，MUST NOT 经由第二块缓冲整体搬运
      （用例 `cached_incremental_rebuild_matches_full_build`）

### Requirement: 构建走哪条路径 MUST 由构建过程回报

实例构建走增量还是全量，MUST 只在构建过程里判定一次，并由构建过程把结果回报给调用方。
调用方 MUST NOT 用自己那套条件重新推断：两处条件只要不完全等价（掩码全脏、掩码短于
行数、图集搬迁、缓存尺寸不符），推断就会与实际不一致，而不一致的后果是拿全量重建的
实例去做稀疏上传——干净行的槽位留在上一帧的陈旧实例上，画面出现错字或错位。

构建过程 MUST 回报「走了增量」与「整帧重建」两种结果；转换失败 MUST 与这两种结果区分
开，调用方 MUST 据此报错而不是继续出帧。

#### Scenario: 增量帧回报增量

- **WHEN** 一帧只标记一行脏，且缓存尺寸、图集代际与掩码都匹配
- **THEN** 构建 MUST 回报走了增量路径（用例 `cached_incremental_rebuild_matches_full_build`）

#### Scenario: 降级为全量时回报全量

- **WHEN** 缓存的图集代际已陈旧、掩码短于行数、缓存尺寸不符，或预热引发图集搬迁
- **THEN** 构建 MUST 回报整帧重建，调用方 MUST 据此放弃稀疏上传
      （用例 `cached_stale_atlas_generation_forces_full_rebuild`、
      `cached_degraded_input_falls_back_to_full_rebuild`、
      `incremental_frame_falls_back_to_full_rebuild_when_warm_evicts`）

#### Scenario: 转换失败与路径判定区分开

- **WHEN** 单元数据无法转换为实例
- **THEN** 构建 MUST 回报失败，调用方 MUST 返回错误，MUST NOT 把它当成全量重建继续出帧
