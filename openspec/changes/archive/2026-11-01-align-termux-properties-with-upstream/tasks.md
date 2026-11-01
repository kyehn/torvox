# Tasks

## 1. 启动软键盘默认值对齐上游

- [x] 1.1 把 `TermuxProperties.hideSoftKeyboardOnStartup` 的默认值改为 `false`，并按事实
      重写字段注释（默认值来自上游 `TERMUX_DEFAULT_FALSE_BOOLEAN_BEHAVIOUR_PROPERTIES_LIST`）；
      验证 `TermuxPropertiesParserTest` 中「空文件等于全部默认」仍成立
- [x] 1.2 更新解析器测试里断言旧默认值的用例，并补一条「显式写 `true` 时不弹」的断言；
      验证 `./gradlew :app:testDebugUnitTest --tests 'terminal.emulator.settings.*'` 全绿

## 2. 逐键测试

- [x] 2.1 新增逐键测试类，逐键断言 20 个键的默认值与上游一致（布尔、整数区间、浮点区间、
      枚举取值、`shortcut.*` 的空映射）；验证新测试类单独运行全绿
- [x] 2.2 为每个可写错的键补「非法取值只丢自己那一项并留痕」的断言；验证同一测试类全绿

## 3. 规范同步

- [ ] 3.1 归档本变更，使 `openspec/specs/termux-configuration` 的「启动时的软键盘」条款
      反映新默认值；验证 `openspec validate --all --strict` 全绿
