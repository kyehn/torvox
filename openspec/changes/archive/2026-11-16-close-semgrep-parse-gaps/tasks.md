# 任务

- [x] 复现 semgrep 的解析缺口并逐文件记录被跳过的区域数量
- [x] 确认 cucumber 按正则匹配步骤文本、不校验注解关键词类别（仓内已有反例佐证）
- [x] 11 个步骤定义文件的步骤注解改用 `io.cucumber.java.en.*`
- [x] 重跑 semgrep：解析缺口归零，且新增被扫区域内无规则命中
- [x] 跑 `spotlessCheck`、`assembleDebugAndroidTest` 与 `testDebugUnitTest` 验证改动
- [x] 编写提案与两个能力的规约增量
- [x] 校验规约增量并保持既有能力全绿
- [x] 归档本 change 并同步主规约
