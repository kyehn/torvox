## ADDED Requirements

### Requirement: 引导归档的来源路径必须限定在应用私有目录内

引导归档可由外部 Intent 指定来源路径（仅 debug 构建响应，供仪器化测试预置归档）。
该路径 MUST 落在应用私有目录（`filesDir` 与 `cacheDir`）内：归档内容会被写进
`files/usr` 并以 shell 身份执行，放开任意路径等于让任何应用借本应用安装自己准备的
二进制。判据 MUST 取规范路径的包含性（`..` 段与站外符号链接都归一到真实目标），
MUST NOT 用字符串前缀；检查失败 MUST 拒绝，MUST NOT 落回宽松路径。

#### Scenario: 私有目录内的归档可安装

- **WHEN** 外部 Intent 指定的归档路径落在应用私有目录内
- **THEN** MUST 照常安装

#### Scenario: 私有目录之外的归档被拒绝

- **WHEN** 指定的归档路径落在应用私有目录之外（含 `..` 逃逸与同前缀兄弟目录）
- **THEN** MUST 拒绝该安装请求，MUST NOT 读取或解压该归档

#### Scenario: 拒绝时不回显路径

- **WHEN** 安装请求因路径越界被拒
- **THEN** 日志 MUST NOT 出现该路径本身
