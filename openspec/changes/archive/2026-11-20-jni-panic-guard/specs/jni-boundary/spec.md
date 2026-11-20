## ADDED Requirements

### Requirement: JNI 导出必须把 panic 转成 Java 异常

Rust 侧导出给 JVM 的每一个函数（`Java_*`）MUST 经 `jni_export_guard!` 守卫：panic 与
Rust 错误 MUST 转为 Java `RuntimeException`，进程 MUST 存活。

panic 越过 `extern "system"` 边界会 abort 整个进程：没有 Java 异常、没有崩溃处理器、
现场只剩 tombstone。守卫把这件事降级为「一次可归因的调用失败」。

失败值 MUST 由调用点声明，MUST NOT 取类型默认值：渲染导出的失败值 `-1` 与「空闲帧」的
`0` 语义不同，取默认值会把 panic 伪装成一帧无输出，渲染线程据此继续空转。

#### Scenario: 导出函数内部 panic

- **WHEN** 任一 JNI 导出函数体内发生 panic
- **THEN** 该 panic MUST 被守卫接住并转为 Java `RuntimeException`，进程 MUST 存活
- **AND** MUST NOT 出现 abort（SIGABRT）导致的进程退出

#### Scenario: 失败值与调用点声明一致

- **WHEN** 守卫把失败转成异常
- **THEN** 返回值 MUST 是该导出函数声明的失败值，MUST NOT 是类型默认值

#### Scenario: 加载期钩子不在守卫范围

- **WHEN** `JNI_OnLoad` 这类由 VM 在加载期调用、没有 `Env` 的导出
- **THEN** 它 MUST NOT 被要求经守卫，也 MUST NOT 被下述静态规则命中

### Requirement: panic 文本必须可读

守卫 MUST 把 panic 载荷转成可读文本：字符串载荷取其原文，其余载荷给出占位说明。
MUST NOT 把所有 panic 都报成同一句「非字符串载荷」——那等于把最该看清的诊断抹掉。

panic 载荷的销毁 MUST NOT 发生在解卷路径上：载荷元素的 `Drop` 若在解卷中再次 panic，
Rust 会直接 abort，守卫连 Java 异常都交不出来。

#### Scenario: 字符串载荷的 panic

- **WHEN** 导出函数以字符串（`&str`／`String`）为载荷 panic
- **THEN** 抛出的 Java 异常消息 MUST 含该字符串原文

#### Scenario: 非字符串载荷的 panic

- **WHEN** 导出函数以非字符串载荷 panic
- **THEN** 抛出的 Java 异常消息 MUST 说明载荷不是字符串，MUST NOT 谎称载荷内容

### Requirement: JNI 导出的守卫约束必须由静态扫描固化

「导出函数经守卫」这条约束 MUST 由仓库内的静态扫描规则表达，使新增导出绕过守卫时
扫描 MUST 命中，而不是只依赖人工复核。

规则 MUST NOT 把非 JNI 导出纳入范围（库内的普通 `pub` 函数没有「越过 FFI 边界」的
问题），MUST NOT 把加载期钩子纳入范围。

#### Scenario: 新增导出绕过守卫

- **WHEN** 新增一个 `Java_*` 导出函数而未使用 `jni_export_guard!`
- **THEN** 静态扫描 MUST 命中该处并报错

#### Scenario: 合规导出不被命中

- **WHEN** 导出函数以 `jni_export_guard!` 作为函数体第一条语句
- **THEN** 静态扫描 MUST NOT 命中该处

#### Scenario: 非 JNI 导出不被命中

- **WHEN** 库内新增一个普通的 `pub` 函数
- **THEN** 静态扫描 MUST NOT 命中该处
