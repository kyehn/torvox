# Tasks

## 1. 解析

- [x] 1.1 `ExtraKey.popup` 改为 `ExtraKey?`，默认布局的 DRAWER 用嵌套按键表达 PASTE
- [x] 1.2 解析器支持对象形式的 popup（可带 `macro`）
- [x] 1.3 `key` 与 `macro` 同时出现、popup 套 popup、popup 不是按键：只丢自己那一项并留警告

## 2. 键栏

- [x] 2.1 引入 `KeyAction`（编码器键码 / 特殊动作 / 字面量文本 / 宏）
- [x] 2.2 主键与 popup 共用同一条派发路径，长按不连带触发主键
- [x] 2.3 「能不能渲染」与「渲染成什么」合并到 `resolveKey` 一处判定

## 3. Ctrl+空格

- [x] 3.1 `ctrl-space-workaround` 消费下按与抬起两路事件，只发一次 NUL
- [x] 3.2 原生侧锁住「Ctrl+空格无平台文本时仍是 NUL」

## 4. 测试与文档

- [x] 4.1 解析器用例：对象 popup、不可用 popup、`key` 与 `macro` 同时出现
- [x] 4.2 键栏用例：popup 的键名/字面量/宏/特殊键，长按不连带触发主键
- [x] 4.3 拆分过长的 requirement，popup 语义单独成条
- [x] 4.4 `docs/specification/config.md` 与 `PROHIBITED.md` 对照现状更新
