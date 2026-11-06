# Proposal

## Why

捏合缩放给出的字号**不落在调节条的档位网格**上，两者其实是两套取值集合：

- 调节条：`FONT_SIZE_MIN_SP`（4sp）起、每档 `FONT_SIZE_STEP_SP`（2sp）、上界
  `fontSizeMaxSp` 按步长向下取整，故它能给出的全部取值都在「4sp 起、步长 2sp」的
  网格上。
- 捏合：`zoomFontSize` 只把 `基准字号 × 累计缩放因子` 钳到区间上下限，**不按步长
  取整**，故手势可以给出 17.6sp 这种调节条给不出的字号。

后果链条是同一条「范围与实际可设范围不一致」：

1. 捏合落定后落盘的字号（如 17.6sp）不在档位网格上；
2. 用户进设置页，滑块读数按整数打印（`"%.0f"`）显示「18 SP」，而实际渲染与落盘
   的是 17.6sp——展示与生效分家；
3. 滑块拇指停在非档位位置（Material `steps` 的离散档位与 17.6 不重合），用户只是
   触碰一次滑块，控件按指针位置换算出的档位值就被写回，用户字号被静默改动。

`upstream-alignment` 早已把「双指缩放步长」登记为「已有去向：`font-selection`（步长
取整与捏合上下界同源，调节条末端、捏合末端与落盘值三者一致）」，但实现里只有
「捏合上下界同源」，**「步长取整」从未落地**——规范声明的归因与实现不一致。

## What Changes

- `SettingsRepository.clampFontSize` 由「只钳区间」改为「收敛到**可设档位**」：
  区间钳位后按 `FONT_SIZE_STEP_SP` 落到最近的档位（4sp 起、步长 2sp 的网格）。
  该函数是调节条展示、捏合预览、落盘值生效与设置页展示的**唯一**收敛入口，
  四者因此对同一台设备给出同一个值。
- 捏合手势的字号换算改走同一入口：`zoomFontSize` 返回值落在档位网格上，
  预览值与落定值同源（手势结束不再从预览值跳到落盘值）。
- 手势终结的落定判定改用**收敛后**的字号与基准比较：基准与终值同为档位值，
  真实变化至少相差一整档，故「未变化」可由相等直接判定，不再需要未钳位的原始值
  与容差阈值。
- 补回归用例：钉住「捏合给出的每一档都在调节条档位网格上」「捏合落定值 =
  调节条可给出的值」「区间内的合法档位不被挪动」。

## Capabilities

- `font-selection`（增量）：可设字号必须落在步长档位网格上，捏合与调节条同源。

## Impact

- `android/app/src/main/java/terminal/emulator/settings/SettingsRepository.kt`
  （`clampFontSize` 收敛语义变更）
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
  （`zoomFontSize`、`onScaleEnd` 的落定判定、`zoomSettledOnNewSize`）
- `android/app/src/test/java/terminal/emulator/ui/TerminalSurfaceLogicTest.kt`、
  `android/app/src/test/java/terminal/emulator/settings/FontSizeRangeTest.kt`
  （用例更新与新增）
- `openspec/specs/font-selection/spec.md`（新增一条 Requirement）
