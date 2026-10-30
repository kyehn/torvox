# Design

## 上下文

网格与字体度量的关系有两处：原生字体管线给出单元格度量（宽、高），网格是
`Surface 尺寸 ÷ 单元格度量` 的取整结果。PTY 的 winsize 用的就是这个网格，而 winsize
决定 shell 怎么排版它的第一个 prompt。

问题出在度量的**时点**，不在算法：`initialSpawnGrid` 要求「度量已就绪」，这个前提
确实满足了，但满足它的时候字体管线里还是原生默认字体——字族、字号与光栅尺度要到
spawn 之后的 `applyRenderSettings` 才下发。

## 决策

### 决策一：字体契约在 spawn 之前落定

字族、字号与光栅尺度在原生侧都只落在**唯一一份字体管线**上，与会话无关：

- `NativeBridge.setFontFamily` 的 `session_id` 未被使用；
- `NativeBridge.setFontSizeInPlace`、`setRasterScale`、`loadFontFile` 同理。

既然如此，它们在会话建立之前就可以下发。`spawnTerminalTimed` 于是先落定字体契约，
再读度量、算网格：

```text
落定字体契约 → 刷新单元格度量 → 算初始网格 → spawn
```

初始 winsize 与最终网格同源，attach 之后的那次 `recomputeGridFromFontMetrics` 算出
同样的网格，零变更，shell 不再收到多余的 SIGWINCH。

### 决策二：`Bridge` 的字体入口不再走会话闸

`Bridge` 的 `onSession` 闸拦的是「会话已销毁」的调用。字体相关的原生导出本就不看
会话 id，闸门只会让它们在会话建立之前**静默失效**——这正是度量取到默认字体的
直接原因。故这四个入口改为直接下发，并以 `NativeBridge.isNativeLoaded()` 取代
「会话是否存在」作为前置判据。

`setFontFamily` 的返回值语义随之微调：null 从「会话在调用间隙未建立/已销毁」变成
「原生库未加载」。调用方（`applyFontSettings`）对 null 的处理本来就是「不计入聚合」，
语义不变。

### 决策三：spawn 与契约下发共用同一份字族

字族来自设置（DataStore）。spawn 前读一次、契约下发后再读一次的话，中途改设置会
让初始 winsize 与最终字族不同源。故字族解析一次后同时传给两处。

## 被否决的方案

- **spawn 之后再补一次「不 resize」的特判**：网格与字形度量不一致本身就是缺陷
  （渲染器按旧网格配新字形光栅化，字大行挤）。特判只会把缺陷藏起来。
- **让 shell 晚一点启动，等字体落定**：PTY 的建立与 shell 的启动是同一动作，无法
  拆开；而延迟启动会拖慢首帧。
- **把字体度量做成「预估」值**：度量必须来自真实字体管线，预估值与真实值之间的
  偏差正是本次缺陷的成因。

## 验证

- `InitialSpawnGridTest#spawn grid equals the grid recomputed with the same metrics`：
  spawn 网格与 attach 之后的重算网格逐字相等。
- `InitialSpawnGridTest#real metrics yield the real grid not the default`（既有）：
  度量就绪时直接建出真实网格。
- `FontFileOverrideInstrumentedTest`（既有）：`font.ttf` 覆盖仍走探测路径并生效。
