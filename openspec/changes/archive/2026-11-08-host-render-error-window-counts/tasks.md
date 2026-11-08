# 任务

- [x] 核对规范对限频诊断的要求：窗口内事件计数 MUST 随那一行交出
- [x] 新增 `RenderErrorLedger`：连续错误数与四类失败诊断的窗口闸门分开
- [x] 瞬时渲染错误改用带计数的闸门，行内加 `transient_errors`
- [x] 渲染异常改用带计数的闸门，行内加 `render_errors`
- [x] 错误后恢复改用带计数的闸门，行内加 `recoveries`
- [x] 输出泵失败改用带计数的闸门，行内加 `poll_failures`
- [x] 瞬时渲染错误的分级口径改为含本次在内（首次仍是 info）
- [x] 移除标题查询外层不可达的 catch
- [x] 新增账本单测并改写瞬时错误分级的用例
- [x] `:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessKotlinCheck` 通过
- [x] 归档本 change 并同步主规约
