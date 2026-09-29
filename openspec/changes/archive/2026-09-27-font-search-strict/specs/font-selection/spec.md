## ADDED Requirements

### Requirement: 渲染侧字体库只常驻三族

渲染路径的字体库 MUST 只常驻主字体（用户 `font.{ttf,ttc,otf}`，为空时用 `fonts.xml`
的 monospace 族）、一个符号族（`fonts.xml` 中既无 `name` 也无 `lang` 的族）与一个
区域族（当前 locale 对应的 `lang` 块）。其余 200 余族是 WebView/UI 用字，渲染永不
触及，MUST NOT 常驻加载。

#### Scenario: 库只含三族

- **WHEN** 字体库构建完成
- **THEN** 面数等于主字体 + 符号族 + 区域族，不含 UI 用字族

### Requirement: 字体列表仅设置页按需构建

完整字体族索引 MUST 只在显示字体列表时构建并缓存至进程结束：族名存在字体的
name 表里，只有读完 `fonts.xml` 声明的全部文件才能得到。渲染路径 MUST NOT 触发该
枚举。索引 MUST 按 `fonts.xml` 文档顺序返回并精确去重，不排序、不归并；
`files/home/.termux/fonts` 投放的字体追加在后。

#### Scenario: 渲染不触发族枚举

- **WHEN** 只启动终端不做字体设置
- **THEN** 族索引日志 `FONT_INDEX` 不出现

#### Scenario: 列表顺序等于文档顺序

- **WHEN** 打开字体列表
- **THEN** 前若干项与 `fonts.xml` 的 family 声明顺序一致

### Requirement: 用户字体路径只追加不覆盖

`loadFontFile` 把自定义字体登记到渲染器时 MUST 追加到已注册路径，MUST NOT 覆盖
`files/home/.termux/fonts` 目录项——覆盖会使目录内其余字体永久不可选。

#### Scenario: 探测 font.ttf 后目录字体仍在

- **WHEN** 先注册 `files/home/.termux/fonts` 目录再探测 `font.ttf`
- **THEN** 额外路径仍含该目录，目录内字体仍可被选中
