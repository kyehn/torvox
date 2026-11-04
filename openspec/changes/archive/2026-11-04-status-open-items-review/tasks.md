# 任务

## 一、逐条落状态

- [x] `kitten icat test.png` 后要 clear 两次：已修（放置生成戳缺失，清屏撤不下虚拟
      放置），判据为 `KittyGraphicsPlacementTest` 四个用例。
- [x] 切换应用返回页面闪烁：已修（开孔覆盖状态改为常驻收集），判据为
      `render-stability` 的对应 Requirement 与仪器用例。
- [x] 侧边面板过宽：已修（去掉 240dp 下限，改为 84% 且上限 320dp）。
- [x] 文档提供者找不出 rollback 文件警告：部分收口——应用侧取证齐备；AOSP 与本仓
      均无 `rollback` 字样，缺原始告警文本，已如实记入「剩余条件」。
- [x] 不能加载 jdwp agent 错误：非本仓缺陷（ART 层，仓内无 JDWP 代码与配置）。
- [x] mali_gralloc 错误：已修待真机（交换链格式只接受三通道以上；告警归因钉成
      可复核事实）。
- [x] `docs/persistence` 重新验证和更新：本轮订正四处与实现相反的结论与失效路径。
- [x] `openspec/specs` 与 `docs/specification` 对齐：48 个规范全部通过严格校验；
      本轮修掉响铃取用台账这一处与实现相反的行。
- [x] `android/benchmark` 性能测试与断言：6 个用例全部带断言，模拟器实跑 6/6 全绿。
- [x] `termux.properties` 全键对齐并逐键测试：CONFIG.md 声明的 20 个键全部实现、
      全部被消费，逐键契约用例齐备。
- [x] `docs/specification` 抽查：BUILD／TESTING／PROHIBITED／DESIGN 四节逐条对照
      实现，结论写进 `status-disposition.md`。

## 二、验证

- [x] `openspec validate --all --strict`：48/48 通过。
- [x] `openspec validate --archived --strict`：新增归档项通过（唯一失败项仍是
      `2026-10-11-terminal-improvements` 的有意保留项）。
- [x] `markdownlint-cli2`：0 issues。
