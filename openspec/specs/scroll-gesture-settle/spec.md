# scroll-gesture-settle Specification

## Purpose

任何滚动手势结束（抬指/取消）后滚动状态必收尾，不残留。

## Requirements

### Requirement: 手势结束收尾滚动状态

`ACTION_UP/CANCEL` 到达时若 `isScrolling` 为真，MUST 将 `isScrolling` 置假、
亚行余量归零并推送渲染线程、发送滚动结束回调；不得重置行偏移（保留用户滚动位置）。

#### Scenario: 慢速拖放抬指

- **WHEN** 用户慢速拖动后抬指（无惯性、无点按回调）
- **THEN** `isScrolling` 为假，像素余量为零，新输出可自动回底

#### Scenario: 取消手势

- **WHEN** 手势被 `ACTION_CANCEL` 中断
- **THEN** 同抬指收尾，不残留滚动状态
