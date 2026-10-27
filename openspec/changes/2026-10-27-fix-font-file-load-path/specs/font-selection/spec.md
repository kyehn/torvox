# font-selection Specification

## MODIFIED Requirements

### Requirement: 字体列表仅设置页按需构建

完整字体族索引 MUST 只在显示字体列表时构建并缓存至进程结束：族名存在字体的
name 表里，只有读完 `fonts.xml` 声明的全部文件才能得到。渲染路径 MUST NOT 触发该
枚举——用户选定字体文件（`loadFontFile`）时路径已知，MUST 直接把文件装进渲染字体
库并按族名取用，MUST NOT 为定位这一个族而枚举全部字体文件。索引 MUST 按
`fonts.xml` 文档顺序返回并精确去重，不排序、不归并；
`files/home/.termux/fonts` 投放的字体追加在后。

#### Scenario: 渲染不触发族枚举

- **WHEN** 只启动终端不做字体设置
- **THEN** 族索引日志 `FONT_INDEX` 不出现

#### Scenario: 装载字体文件不触发族枚举

- **WHEN** 用户选定字体文件（`loadFontFile`）
- **THEN** 族索引 MUST NOT 被构建，该文件 MUST 直接装入渲染字体库并可按族名选中

#### Scenario: 按需构建不改变列表顺序

- **WHEN** 打开字体列表
- **THEN** 前若干项与 `fonts.xml` 的 family 声明顺序一致

## ADDED Requirements

### Requirement: 字体来源未变时不得重建字体管线

登记额外字体路径（`setExtraFontPaths`）与装载字体文件（`loadFontFile`）MUST NOT
重建字体管线。渲染常驻字体库不读额外字体路径，重建得到的库与重建前同一份；重建还会
把主字体族复位成 `fonts.xml` 的 monospace 族、把光栅缩放复位成 1.0，用户看到字体与
字号突然改变。字体源确实变化时，MUST 只失效按 UV 缓存的实例并请求重绘，判据 MUST 与
`setFontFamily` 同一口径（字体面变化才失效）。

#### Scenario: 登记字体目录不改变当前主字体

- **WHEN** 会话已选定某个主字体族，随后登记额外字体路径
- **THEN** 主字体族 MUST NOT 变回 `fonts.xml` 的 monospace 族，光栅缩放 MUST NOT 被
      复位

#### Scenario: 装载字体文件后字号不变

- **WHEN** 用户装载字体文件使主字体族改变
- **THEN** 光栅缩放 MUST 保持不变，且渲染器 MUST 丢弃按 UV 缓存的实例并重绘

### Requirement: 字体文件被替换后按族名选中必须用新装入的面

字体库只增不删：同一路径的字体文件被替换后旧面仍在库里。渲染字体库 MUST 记住用户
选定文件时装入并选中的面，按族名选中时 MUST 用该面，MUST NOT 按族名扫库取首个而选回
旧面——用户看到的是「换了字体却不生效」。

#### Scenario: 替换字体文件后选中新面

- **WHEN** 同一路径的字体文件被替换并重新装入，随后按同一族名选中
- **THEN** 选中的面 MUST 是重新装入的那份，MUST NOT 是替换前留在库里的旧面
