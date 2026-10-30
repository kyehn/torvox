## 1. 放大镜与右键手势配对收起

- [x] 抽 `dismissMagnifier(reason)`，收起与置空收在一处
- [x] 右键分支在 `ACTION_UP`/`ACTION_CANCEL` 收起
- [x] `onDetachedFromWindow` 与手势收尾处改为复用该函数

## 2. 关闭会话的关闭资格在调用前捕获

- [x] `closedActive` 移到 `runtime.closeSession` 之前读取
- [x] 注释写明收集器先于 `withContext(main)` 入队，故块内读取恒为 false

## 3. 弹窗拆除只在失败时兜底

- [x] `dismiss()` 成功即 return，不再无条件 `removeViewImmediate`
- [x] 兜底分支记 warn，消除空 catch

## 4. 引导 URL 编辑落盘失败不再静默

- [x] collect 体内捕获非取消异常并记 error
- [x] 取消照原样重抛（协作式终止不得被吞）
- [x] 补 `TAG` 与 `LogUtil` 导入

## 5. 安装成功负载去掉恒空的诊断串

- [x] 删除成功分支的 `details` 拼装（恒为空串）
- [x] 注释说明非致命 stderr 摘录只在失败分支给出

## 6. 注释与常量

- [x] 选区菜单宽度注释改述固定常量
- [x] 抽屉轻击抑制注释改为引用常量名
- [x] `RENDER_MAX_TRANSIENT_ERRORS` 注释改述日志节流语义
- [x] 删除「不重复输出」的残留注释
- [x] `32` → `DRAWER_EDGE_GESTURE_DP`；`200L` → `SURFACE_READY_RETRY_DELAY_MS`
