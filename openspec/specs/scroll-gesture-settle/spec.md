# scroll-gesture-settle Specification

## Purpose

任何滚动手势结束（抬指或取消）后滚动状态必须收尾，不留残留。`isScrolling` 若在
抬指后仍为真，亚行余量不归零，render-loop 的 `shouldResetScroll` 便会被旧状态
挡住，新输出无法自动回到底部，表现为终端「滚不动了」。收尾只清状态不动行偏移，
以保留用户滚动到的位置。

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

### Requirement: 外接滚轮在未上报时滚动本地回滚

主屏收到鼠标滚轮且应用未启用鼠标上报（编码器返回 false）时，MUST 将轴向增量
交给与触摸相同的滚动管道，MUST 停止进行中的惯性。应用已消费滚轮时 MUST NOT
同时滚动本地回滚；备用屏 MUST NOT 进入本地回滚。

#### Scenario: 普通 shell 中滚轮向上

- **WHEN** 主屏无鼠标上报且存在回滚内容，用户将外接滚轮向上滚动
- **THEN** 视口进入更早的本地回滚，PTY 不收到鼠标序列
