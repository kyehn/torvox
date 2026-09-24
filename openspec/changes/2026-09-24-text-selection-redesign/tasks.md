# Tasks

## 1. 变更文档

- [x] 1.1 写 proposal/design/specs delta/tasks 并 `openspec validate 2026-09-24-text-selection-redesign --type change` 通过

## 2. 死代码与未声明行为清理

- [x] 2.1 删 ModifierBar 第二套选择菜单与 TerminalScreen 死接线、删 `computeMenuPosition`/`MenuPosition` 与 `TerminalScreenMenuTest`、修 SelectionSteps 悬空断言；`testDebugUnitTest`+`spotlessCheck`+`detekt` 通过
- [x] 2.2 删 `expandAndSetSelection`/`SelectionExpander` 链与 mode/rectangle 全通道（Kotlin 与 ffi 同步）、删 `SelectionExpanderTest`；`cargo test` 与 gradle 三件套通过
- [x] 2.3 删 `selectionBackground` 死通道、方向键移锚（裁剪 `SelectionStateTest`）、SmartCopy（删 `SmartCopyTest`）并修过时注释；gradle 三件套通过

## 3. 上游选择语义接入

- [x] 3.1 native 新增 `SelectWordAt`/`SelectLineAt`/`SelectAll` 查询（派生+安装+回传界限）与 gref→绝对坐标反解；cargo 测试（含全选界限不含尾部空行）通过
- [ ] 3.2 Kotlin 长按与多击改调上游接口、删 Kotlin 词/行/视口全选自实现；单测与 `MultiTapSelectionInstrumentedTest` 按上游词边界调整后通过

## 4. 菜单与行为修复

- [ ] 4.1 锚定纯函数（翻转/贴边/无处可放→隐藏）+ 全选后重锚 + 抓柄即隐藏；新单测、`SelectionEspressoTest` 重锚断言、`SelectionDragQuantifiedTest` 隐藏断言通过
- [ ] 4.2 边缘滚动改每次触点移动 1 行（删 30ms 循环）；`SelectionDragQuantifiedTest` 调整后通过
- [ ] 4.3 OSC 8 打开链接回退 `resolveOpenLinkUri` + 单测；`SelectionMenuActionsTest` 通过
- [ ] 4.4 打开文件删位置限制（点击时存在检查与写权限保留）；相关单测通过
- [ ] 4.5 菜单样式改 Material 3 主题属性；`spotlessCheck`+`detekt` 通过

## 5. 总验证与归档

- [ ] 5.1 `cargo fmt --check`+`cargo test`、gradle 三件套、选择相关仪器测试（Espresso/DragQuantified/MultiTap/TapDismiss/cucumber）全绿
- [ ] 5.2 `openspec archive 2026-09-24-text-selection-redesign --yes` 后 `openspec validate --all` 通过并推送
