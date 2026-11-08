# Proposal

## Why

「清除应用数据」当前只删 `prefs` 目录，而设置存储是进程级单例，内部缓存着已读入
的状态。删目录之后：

- 存储仍按旧值对外供给，界面继续显示清除前的字号、主题与引导 URL；
- 下一次写入会把内存里的整份旧状态连同新值一起写回文件，已删的设置逐个复活。

即用户看到的「已清除」提示与实际状态不符，且用户改一个设置就会把全部旧设置
带回磁盘。已按项目规则读过规范目录，并核对 `DataStore` 1.3.0 的字节码：缓存位于
`DataStoreImpl` 的内存缓存，读路径在命中缓存时直接返回，不回磁盘。

修复不需要换存储：先经存储本身把全部键清空（内存态与文件态同时归零），再删目录
即可，两步都是既有能力的直接用法。

## What Changes

- 设置仓库增一个「清空全部设置项」的动作，经 DataStore 自身清空全部键。
- 清除应用数据的流程改为「先清空内容，再删目录」，删除后重建目录的既有行为不变。
- 补单测：清除后各项回落缺省；目录被删后设置仍为空；清除后写入不会带回旧值。
- `settings-propagation` 增一条要求：清除应用数据必须同时清空内存态与文件态。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `settings-propagation`：增一条「清除应用数据必须同时清空内存态与文件态」的要求。

## Impact

- `android/app/src/main/java/terminal/emulator/settings/SettingsRepository.kt`：
  新增清空全部设置项的动作。
- `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`：
  清除应用数据时先清空设置内容。
- `android/app/src/test/java/terminal/emulator/settings/SettingsRepositoryTest.kt`：
  新增回归用例。
- `openspec/specs/settings-propagation/spec.md`：归档时同步该条要求。
