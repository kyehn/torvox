## MODIFIED Requirements

### Requirement: 渲染侧字体库只常驻三族

渲染路径的字体库 MUST 只常驻主字体、一个符号族（`fonts.xml` 中既无 `name` 也无
`lang` 的族）与一个区域族（当前 locale 对应的 `lang` 块）。主字体恒为 `fonts.xml`
的 monospace 族：用户 `files/home/.termux/font.{ttf,ttc,otf}` 覆盖经 `setFontFamily`
按需装入生效，`files/home/.termux/fonts` 投放字体只进字体列表、选中时经
`load_family` 装入；两者 MUST NOT 在建库时抢占主字体位置。其余 200 余族是
WebView/UI 用字，渲染永不触及，MUST NOT 常驻加载。

#### Scenario: 库只含三族

- **WHEN** 字体库构建完成
- **THEN** 面数等于 fonts.xml 等宽主字体 + 符号族 + 区域族，不含用户投放字体与
  UI 用字族

#### Scenario: 投放字体不抢占主字体

- **WHEN** `files/home/.termux/fonts` 目录存在字体文件且用户未选择字体
- **THEN** 主字体仍是 fonts.xml 的 monospace 族，应用正常启动不 abort

## ADDED Requirements

### Requirement: 主字体选择降级梯次

主字体选择 MUST 按序降级：声明的等宽文件按词干精确匹配（分隔符归一，再去空白）→
库内任一等宽面 → 库内首个可用面；每级降级 MUST 输出日志。仅当库内没有任何可用面
（等同 `fonts.xml` 不可用）时才 MUST 输出日志并 abort。`fonts.xml` 缺失或不可解析
MUST 保持 abort（DESIGN 不变）。

#### Scenario: 声明文件被 OEM 改包装

- **WHEN** `fonts.xml` 声明的等宽文件已加载但家族名与词干不匹配
- **THEN** 选择库内任一等宽面并输出警告日志，不 abort

#### Scenario: 全库无等宽面

- **WHEN** 放宽装库后库内仍无等宽面但至少有一个可用面
- **THEN** 选择首个可用面并输出错误日志，终端降级可用

#### Scenario: 库为空

- **WHEN** `fonts.xml` 全量声明文件无一可加载
- **THEN** 输出日志并 abort

### Requirement: 声明等宽字体不可用时放宽装库

最小常驻集构建完成后若不含任何等宽面（OEM 声明的等宽文件缺失或损坏），MUST 一次性
放宽装入 `fonts.xml` 全量声明文件（跳过已在库中的文件），并由进程级缓存承载；
健康设备 MUST NOT 进入该分支。

#### Scenario: 声明文件缺失时启动

- **WHEN** `fonts.xml` 声明的等宽文件在所有平台字体目录中都不存在
- **THEN** 装库放宽到全量声明集并输出错误日志，应用正常启动

#### Scenario: 健康设备不放宽

- **WHEN** 声明的等宽文件正常加载
- **THEN** 常驻集保持三族，不出现放宽日志
