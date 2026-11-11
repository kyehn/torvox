# Proposal

> **实测更正**：本变更更正一条**已写进主规约、但实现上做不到**的要求。
> 原要求写「由**窗口层**涂底色/遮罩」，而窗口层在开孔区域的像素一律被平台抹掉，
> 那条要求自落地起就没有真正生效过——「切换后台返回终端闪烁」正是它留下的。

## Why

`docs/STATUS.md` 的未决问题「切换后台返回终端闪烁」一直没收口。逐层排查后定位到
遮罩**画错了层**，而不是遮罩本身不对：

平台在 `SurfaceView` 绘制时先给应用窗口**打洞**（`SurfaceView.draw` →
`clearSurfaceViewPort` → `canvas.punchHole`），洞里**窗口层**画过的像素一律被抹掉。
实测（模拟器，终端 Surface 铺满内容盒）：

- 把组合层的内容盒整个涂成红色 → 屏幕上只有 `SurfaceView` 之外的边距是红色，
  洞里始终是空的；
- 把同样的红色画进 `TerminalSurface.onDraw` → 洞里立刻变红。

即：洞只能由 `SurfaceView` 自己填。`SurfaceView.draw` 的顺序是「先打洞、再
`super.draw`（也就是 `onDraw`）」，故 `onDraw` 的内容落在洞**之后**，能盖住原生
surface 之上的空档；而组合层画的落在洞**之下**，必被抹掉。

原实现的遮罩（切后台时回读的离开画面）内容是对的，只是画在了组合层，于是从未
出现在开孔里；用户看到的仍是「闪一下」。

## What Changes

- 开孔遮罩改由 `TerminalSurface.onDraw` 画：原生 surface 尚无缓冲时画遮罩（离开时的
  画面），有缓冲时不画。
- 组合层不再画遮罩，也不再按「有没有缓冲区」切换底色——那层底色在洞里必被抹掉，
  留在组合层的唯一作用是给**边距**（`terminal-margin-*`）上色。
- 「有没有缓冲区」的判据以**运行期的事实**为准（谁呈现过帧），由视图在绘制时现取；
  组合层只负责把该事实推给视图，好让视图在事实翻转时及时重绘（撤下遮罩晚一帧就是
  用户看到的那一下闪）。
- 遮罩与当前 surface 尺寸不一致时不用（错位的位图比没有遮罩更糟）；没有可复现的
  上一帧时铺终端底色，不让开孔透出后方桌面。
- 主规约 `render-stability` 的两条相关要求改写为「由 `SurfaceView` 自己在
  `onDraw` 里遮」，并写明窗口层画不上的实测依据。

## Capabilities

### Modified Capabilities

- `render-stability`：开孔遮罩的**绘制位置**（由 `SurfaceView` 的 `onDraw` 画，
  不是组合层），以及「有没有缓冲区」的判据来源。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`
- `android/app/src/androidTest/java/terminal/emulator/ui/TerminalHoleCoverInstrumentedTest.kt`
- `openspec/specs/render-stability/spec.md`

## 取舍与缺口

- 遮罩画在 `onDraw` 里即盖在原生 surface 之上，故必须有「原生尚无缓冲」这道闸，
  否则遮罩会把终端内容一起盖住（实测去掉闸后开孔里只剩一块平底色）。闸的判据是
  运行期的事实，不是视图的猜测。
- 原生 surface 的 warmup 会先呈现保留帧、渲染循环的首帧随后才到：两者之间
  `surfaceBufferPresent` 仍为假，遮罩会短暂盖住那一帧。两者同源（都来自帧累加器），
  故盖住的那一帧与遮罩内容一致，用户看不到跳变。
- 遮罩的像素是原生缓冲的原值，屏幕合成管线对开孔区域还有一次色彩变换（实测
  `screen = 0.878 × buffer + 31`）：同一块区域里遮罩与原生内容一起经过该变换，故
  两者在屏幕上的颜色一致，撤下遮罩时无色差。
- `docs/STATUS.md` 首行声明禁止修改本文件，故未在本次变更中改写该清单；该条未决
  问题的收口口径写在本文与主规约里。
