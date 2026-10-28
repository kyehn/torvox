# Proposal

## Why

组字区的其他写入口没有走 `setComposingText` 已有的重放路径，于是各自带一个缺陷：

- `replaceText`（API 33+）直接按 `ComposingDiff.reconcile` 的差分发退格，既没有先把
  光标挪到旧组字区末尾，也没有在重放后把光标落回目标位置。组字光标停在组字区中间
  时，退格删掉的是组字区**之前已提交**的内容；重放后 PTY 光标停在组字区末尾，而
  输入法侧的光标在中间，两者从此错位。
- `replaceText` 的光标落点按「相对整段基线末尾」计算，而平台口径是「相对**被插入
  文本**」（`newCursorPosition > 0` 相对被插入文本末尾减一，`<= 0` 相对其开头）。
  范围替换落在组字区中间时，光标被甩到基线末尾而非插入文本之后。
- `setComposingRegion` 的平台语义是「只重新标定哪一段正在组字，**一个字符都不改**」，
  而现有实现把组字基线截断成该区间再按差分重放：组字区**之外**的内容被从终端上
  删掉了——该方法自己的注释写着「区间外的已提交内容必须原样保留」，实现与注释相反。

平台口径的来源：AOSP `InputConnection` 的接口文档（`replaceText` 与 `commitText`
的 `newCursorPosition` 说明）与 `BaseInputConnection.setComposingRegion` 的实现
（只改 composing span，不动文本）。

## What Changes

- `replaceText` 改走既有重放计划：光标先对齐到旧组字区末尾，重放差分后再落到目标
  组字光标，与 `setComposingText` / `deleteSurroundingText` 同一条路径。
- 新增 `ComposingBaseline.replacedCursor`：按平台口径计算范围替换后的绝对光标
  （相对被插入文本），越界值钳入替换后的文本范围。
- `setComposingRegion` 改为零字节、零光标移动：平台语义下重锚不改文本。本仓的组字
  基线整体即组字区（差分出口以整段基线为单位），故只受理覆盖整段基线的重锚；
  子区间重锚要求后续组字只替换该子区间，而本仓无从处置区间外的已提交内容——如实
  拒绝，不发字节。
- 移除随之失去调用方的 `ComposingBaseline.compose`（死代码），其光标口径由
  `resolveCursor` 单独承载并单测。
- 新增单测覆盖：范围替换的光标落点、子区间重锚被拒、整段重锚零字节、范围替换的
  重放计划。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ime-editor-contract`：范围替换的光标落点纳入「组字改写的光标落点必须在组字区内」；
  新增「组字区重锚不改文本」的要求；更正「已提交词的改写经重锚送达终端」的措辞，
  使其与实现一致。

## Impact

- `android/app/src/main/java/terminal/emulator/input/ComposingBaseline.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `android/app/src/test/java/terminal/emulator/input/ComposingBaselineTest.kt`
- `android/app/src/test/java/terminal/emulator/ui/ComposingDiffTest.kt`
- 不改 `docs/specification/`、`docs/STATUS.md` 等保护文件，不引入新依赖。
