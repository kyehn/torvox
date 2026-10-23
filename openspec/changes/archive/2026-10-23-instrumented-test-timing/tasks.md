# Tasks

- [x] 1.1 定位全量套件的两处红灯：解析 `androidTest-results` 的 XML 与 HTML 报告
- [x] 1.2 Kitty 滚动判据：找出期望值与实测值不符的根因（滚动量不是 3 行）
- [x] 1.3 Kitty 判据改造：滚动改走 VT 解析器、位移期望值改用渲染帧的回滚长度
- [x] 1.4 Kitty 用例复跑：计量 `scrolled_rows=3`、`scroll_shift_px=51`、像素数不变
- [x] 1.5 UiAutomator 根因：读全量运行里该用例的 logcat，取证点击时序
- [x] 1.6 UiAutomator 交互协议改造：等节点边界停稳再点，点后校验并重试
- [x] 1.7 失败信息补上无障碍树摘要，便于下次直接看出「抽屉开没开」
- [x] 1.8 全量套件复跑：`231 例 / 0 失败`
- [x] 1.9 `openspec validate --all --strict` 通过
