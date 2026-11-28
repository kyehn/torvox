# 任务

- [x] 定位分叉：转发路径 `wheelDragStep` 用 `accumulatorPx + distanceY`，与本地回滚的
      `- distanceY` 相反，且把正值映射为滚轮上
- [x] 改为同一语义与同一累加约定（正 = 更旧的待消化位移）
- [x] 回归测试：方向两向、截断趋向零的对称性、转发与本地回滚同向
- [x] 变异验证：恢复旧符号判红 7 例；改 floor 取整判红对称性用例
- [x] 仪器用例改判 SGR 65（ghostty `Button::Five`）
- [x] 规范补上「转发滚轮与本地回滚同向」的要求与场景
- [x] `:app:testDebugUnitTest --tests "terminal.emulator.ui.*"`、`:app:spotlessCheck`、
      `:app:detekt` 通过
