## ADDED Requirements

### Requirement: 原生日志在首个 JNI 调用之前就位

原生日志 MUST 由 `JNI_OnLoad` 安装，使 logger 在任何 JNI 方法之前就位。
Kotlin 侧 MUST NOT 以独立线程异步安装日志：启动数秒内的致命退出会抢在安装之前，
经 `log` 门面的原因全部被静默丢弃，现场只剩无符号 tombstone（真实机启动崩溃即此情形）。
MUST NOT 保留已无用途的日志初始化导出与调用方。

#### Scenario: 启动即致命退出仍留下原因

- **WHEN** 进程启动后立即触发原生致命退出
- **THEN** logcat 中出现该退出写出的错误行，而不只是 tombstone

### Requirement: 字体致命退出单一出口且原因指向真实前提

字体侧所有致命退出 MUST 经 `font_db::fatal(reason) -> !` 唯一出口：以 `FONT_FATAL`
为 target 输出 ERROR 后 `abort`，使同类崩溃一次 grep 即可归因。`reason` MUST 陈述真实
前提，MUST NOT 把「字体文件装入失败」写成 `fonts.xml` 问题。
`font_db::fatal` MUST NOT 依赖 `android` 模块（层方向由 semgrep `no-android-in-render`
守卫），日志安装的时序保证由 `JNI_OnLoad` 承担而非直接写 logcat。

#### Scenario: 库空的原因指向字体文件

- **WHEN** `fonts.xml` 可解析但其声明的字体文件一个都装不进 `fontdb`
- **THEN** 致命原因指向字体文件装入失败，`FONT_FATAL` 行不归因 `fonts.xml`

#### Scenario: 等宽字体只解析一次

- **WHEN** 同一进程内创建管线并随后多次以空族名设置字体
- **THEN** `fonts.xml` 只被读盘解析一次，其结果由进程级缓存承载

## MODIFIED Requirements

### Requirement: 主字体选择降级梯次

主字体选择 MUST 按序降级：声明的等宽文件按词干精确匹配（分隔符归一，再去空白）→
库内首个覆盖基本拉丁的等宽面（charmap 探测 `'m'`，跳过 emoji/符号这类被标记为
等宽却无拉丁字形的面）→ 库内首个覆盖拉丁的面 → 库内首个可用面；每级降级 MUST
输出日志。仅当库内没有任何可用面时，MUST 经 `font_db::fatal` 输出日志并 abort。
`fonts.xml` 缺失、不可解析或未声明等宽族 MUST 保持 abort（DESIGN 不变），
并同样经该唯一出口退出。

#### Scenario: 声明文件被 OEM 改包装

- **WHEN** `fonts.xml` 声明的等宽文件已加载但家族名与词干不匹配
- **THEN** 选择库内首个覆盖拉丁的等宽面并输出警告日志，不 abort

#### Scenario: 无拉丁字形的等宽面被跳过

- **WHEN** 库内等宽面不含拉丁字形（如 emoji 字体）
- **THEN** 该面被跳过，选择首个覆盖拉丁的面

#### Scenario: 全库无等宽面

- **WHEN** 放宽装库后库内仍无可用等宽面但至少有一个覆盖拉丁的面
- **THEN** 选择首个覆盖拉丁的面并输出错误日志，终端降级可用

#### Scenario: 库为空

- **WHEN** `fonts.xml` 全量声明文件无一可加载
- **THEN** 经 `font_db::fatal` 输出 `FONT_FATAL` 行并 abort

#### Scenario: fonts.xml 未声明等宽族

- **WHEN** `fonts.xml` 可解析但只含 `lang` 族与无名族
- **THEN** 无主字体候选，经 `font_db::fatal` 输出 `FONT_FATAL` 行并 abort
