## ADDED Requirements

### Requirement: 编译平台可声明同级别次版本号

依赖的 AAR 元数据要求高于 37 的**同级别次版本号**（如 api 37.1）时，MUST 以次版本号
声明编译平台（如 `compileSdkMinor`），MUST NOT 因此抬高 API 级别；次版本号 MUST NOT
改变 `minSdkVersion` 与 `targetSdkVersion`，MUST NOT 改变应用可用的 API 面。次版本号
MUST 在构建脚本中写明触发原因。

#### Scenario: 依赖要求同级别次版本号

- **WHEN** 依赖的 AAR 元数据要求编译于高于 37 的次版本号（如 api 37.1）
- **THEN** 构建 MUST 以同级别的次版本号声明编译平台，MUST NOT 抬高 API 级别
- **AND** 构建脚本 MUST 写明触发该次版本号的依赖与原因
