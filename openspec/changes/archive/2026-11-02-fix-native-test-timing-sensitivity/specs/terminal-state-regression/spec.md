## ADDED Requirements

### Requirement: 压力用例不得把机器速度当判据

回滚深度、回滚顺序、命令批边界与查询应答期限这类**压力用例**，其判据 MUST NOT
依赖「机器足够快」这一前提。用例 MUST 以「被测语义是否成立」为唯一判据，
MUST NOT 因整幅网格重建、命令批尾发布等与被测语义无关的成本而判红。

实现口径：同一段输入 MUST 以整段字节一次入队的方式写入，MUST NOT 逐行
`vt_write`；排空 MUST 以 `flush_with_timeout(期限)` 等待并显式断言排空完成；
期限 MUST 只用于把「VT 线程卡死」与「机器慢」分开，MUST NOT 参与被测语义判定。

#### Scenario: 同一份输入整段写与逐行写得到相同的回滚状态

- **WHEN** 同一段行文本分别以逐行 `vt_write` 与整段 `vt_write` 写入两个终端
- **THEN** 两者的回滚深度与回滚行内容 MUST 相同——整段写 MUST NOT 改变解析结果
      （上游解析器跨调用保持状态，分片与整段等价）

#### Scenario: 压力用例在慢构建配置下不判红

- **WHEN** 上游 Zig 库以 Debug 优化模式构建（即 cargo dev profile 的默认；
      `libghostty-vt-sys` 的 `zig_optimize_mode()` 读 `DEBUG` 环境变量，
      Debug 下 `Screen.cursorDownScroll` / `PageList.grow` 每次滚动都跑
      `assertIntegrity()`，处理成本随构建配置相差数倍至数十倍）
- **THEN** 回滚深度、回滚顺序、命令批边界与查询期限类用例 MUST 全部通过，
      MUST NOT 因整幅网格重建的成本而超时判红

#### Scenario: 排空未完成时断言直指排空

- **WHEN** 压力用例的排空未在期限内完成
- **THEN** 失败消息 MUST 说明「排空未完成」及其后果（状态只反映已处理的部分），
      MUST NOT 只报告一个含混的深度偏差

#### Scenario: 放宽期限不掩盖被测缺陷

- **WHEN** 被测的语义确实不成立（命令批无界使通道恒非空、回滚不增长、
      查询无应答）
- **THEN** 用例 MUST 判红，MUST NOT 因期限放宽而被放过——被测的失败模式在任何
      期限下都等不到排空，故放宽期限 MUST NOT 削弱判别力
