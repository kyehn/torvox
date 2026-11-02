# 任务

## 一、缺陷核定

- [x] 复现：`\\u12` 与 `\\user` 使 `Properties.load` 抛
      `IllegalArgumentException: Malformed \uxxxx encoding.`。
- [x] 核实根因：降级正则不区分反斜杠奇偶，把「不是转义符的 `u`」改写成了转义符。
- [x] 核实影响面：异常一路穿过 `TermuxPropertiesParser.parse` →
      `TermuxPropertiesRepository.loadOnce` → `TerminalRuntime.start()`，整份配置作废
      且启动被打断，与「读失败不阻断启动」的既有约定相悖。

## 二、实现

- [x] `MALFORMED_UNICODE_ESCAPE`：改为 `(\\+)u(?![0-9a-fA-F]{4})`，把反斜杠连成一组
      捕获，据其奇偶判定 `u` 是不是转义符。
- [x] `downgradeMalformedUnicodeEscapes`：奇数个反斜杠时删掉引出转义的那一个、
      保留成对反斜杠；偶数个时原样返回。
- [x] `PropertiesFile.parse`：改用该函数做降级。

## 三、回归钉

- [x] `an escaped backslash before u does not invalidate the file`：`\\user` 不再
      作废整份文件，值为 `\user`。
- [x] `an escaped backslash before a malformed unicode escape does not invalidate the
      file`：`\\u12` 不再作废整份文件，值为 `\u12`。
- [x] 既有解析用例全部仍通过（非法 `\u12` 降级为 `u12`、非 ASCII 值、重复键留痕、
      越界回退、extra-keys 迷你语法均未受影响）。

## 四、验证

- [x] `:app:testDebugUnitTest --tests "terminal.emulator.settings.TermuxPropertiesParserTest"`
      → 45 tests, 0 failed（修复前新增的 2 条判红，抛 `IllegalArgumentException`）。
- [x] `:app:testDebugUnitTest` 全量。
- [x] `detekt` 与 `spotlessCheck`。
- [x] `markdownlint-cli2` 与 `openspec validate --all --strict`。
