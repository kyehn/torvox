# 任务

- [x] 定位分叉：`wheelForwarded` 被本次编码结果覆盖，转发过也可被清掉
- [x] 转发标记改为只升不降（`wheelForwardedState`）
- [x] 回归测试：转发过后编码失败仍保持转发态
- [x] `./gradlew --continue detekt spotlessCheck testDebugUnitTest` 通过
- [x] 规范补上「转发标记在手势内只升不降」的场景
