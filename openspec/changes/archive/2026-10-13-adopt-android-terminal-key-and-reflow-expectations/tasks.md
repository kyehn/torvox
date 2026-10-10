## 1. 按键折叠期望

- [x] 1.1 逐键比对 `TerminalInputEncoder.controlByteForCodePoint` 的分支与对照项目的字面期望
- [x] 1.2 采纳数字行（2-8、空格）与标点行的字面期望，改锚外部标准与既有实现谱系
- [x] 1.3 Ctrl+1/9/0 不入该表（走 `CSI 27;5;code~`，由既有用例覆盖），在注释里写明理由

## 2. 重排回归

- [x] 2.1 核实现有三条 resize 用例均不检查屏幕内容
- [x] 2.2 新增往返逐字相同的断言，并用实测确认中间宽度确实发生折行（有判别力）
- [x] 2.3 新增「宽字符不进末列」断言，判据取末列而非每行字符数奇偶
- [x] 2.4 新增「缩小再放大后可见区恰好三行」断言，覆盖回滚区残留

## 3. 对照结论

- [x] 3.1 逐仓核对测试体系的断言形态，不比数量
- [x] 3.2 记录采纳项与不采纳项及其判据
- [x] 3.3 把「按覆盖面缺口采纳、须验证判别力」写进 `upstream-alignment` 规范

## 4. 验证

- [x] 4.1 `cargo test --test reflow_round_trip` 三项通过
- [x] 4.2 `TerminalInputEncoderTest` 通过
- [x] 4.3 `cargo clippy --workspace --all-targets -- --deny warnings` 无告警
