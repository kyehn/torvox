# Tasks: full-closure-v3

- [x] CJK保序+Serif惩罚+token边界+outline缓存+多字符投票
- [x] surfaceCreated 0-size守卫+ON_RESUME清暂停
- [x] IME_TOGGLE 80ms+SETTLE 3帧+导航栏去重+可见性切换清理
- [x] imeFollow提取+ModifierBar去重
- [x] 依赖更新+文档+verification
- [ ] cargo test --workspace 997 pass (verified 997)
- [ ] spotlessCheck/detekt BUILD SUCCESSFUL (verified)
- [ ] 模拟器HOME往返零黑屏+IME 0丢帧+CJK <400ms+抽屉×3有效 (verified via log/screencap)
- [ ] 4轮双审连续PASS

## Acceptance Criteria

- `cjk_fallback_names()[0]` == 实际渲染首项 (log sans -15 > serif -47)
- 中文14字首屏<400ms, 二次<16ms
- IME 0丢帧, 无跳跃闪烁, 文本不压扁
- HOME往返3次零黑屏
- 抽屉按钮×3有效
