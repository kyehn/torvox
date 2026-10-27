# Proposal

## Why

全量 jscpd 扫描（`native/src` + `android/app/src/main`）测得 53 处克隆（21 处生产代码 / 32 处测试代码），Rust 生产代码重复率 0.62%、Kotlin 0.087%。其中若干处是同一段判定或同一段拷贝逻辑被抄写两遍：

- 图集 UV 换算在 `cell_builder` 与字体叠加层路径各写一遍；
- JNI 字节数组读取在 `feedTerminal` 与 `encodePaste` 各写一遍；
- 原子标志 → JNI 布尔映射在备用屏与鼠标上报两个导出各写一遍；
- RGB → RGBA 展开在 Kitty 图像直传与 PNG 解码兜底各写一遍；
- ESC 透传在 `ReadScan` 的两条分支各写一遍。

这类克隆不是「长得像」，而是**同一份判定**：两处各写一遍时，改动只落在一处，另一处即静默分叉（如叠加字形贴错图集位置、粘贴字节读失败被当成空输入、Kitty 图像少一个通道）。

## What Changes

- 提取共享 helper，让上述判定只此一处（行为不变）。
- 收敛渲染状态锁获取与单元绘制调用：两条渲染路径（阶段 1 新帧 / 阶段 3 空闲帧）共用同一段样板。
- 收敛选区导出：词选区与全选共用同一个 JNI 导出包装。
- 收敛图集像素拷贝：掩码与彩色两条分支共用同一段边界判定与写入。
- 收敛 wgpu 绑定组创建：单元绑定组与 Kitty 图形绑定组共用同一个创建函数。
- 收敛 VT 查询应答的参数打包：`process_query` 与 `drain_queries` 共用同一个上下文结构。
- 收敛 Kotlin 修饰键状态读取：输入法提交与硬键盘多码点提交共用同一个读取点。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无（纯内部重构，产品行为不变）

## Impact

- `native/src/render/**`、`native/src/android/ffi/**`、`native/src/terminal/**`、`android/app/src/main/java/**` 纯重构，行为不变。
- 不改 `docs/specification/`、`docs/STATUS.md`、`.github/`、`scripts/`、`*.gradle.kts` 等保护文件。
- 不引入新依赖，不 vendor 任何文件。
