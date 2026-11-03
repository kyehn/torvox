禁止对本文件进行任何修改

---

## 未被解决的问题

基于 `86232858b9aff31c3cb28a5a0dc53c1571be2284`

- 输入法未弹出时，修饰键栏底部存在过高的空区域背景
- 使用 `kitten icat test.png` 后，输入 clear 回车不会清除图像，图像变大，文本被遮挡，再次盲输入 clear 回车被清除
- 切换应用返回页面闪烁
- 侧边面板过宽，布局不合理
- 文档提供者找不出 rollback 文件警告
- 不能加载 jdwp agent 错误
- mali_gralloc 错误
- 输入光标遮盖文本时可见性差，和 termux 效果不一致
- docs/persistence 内容需要重新验证和更新
- openspec/specs 目录文档需要根据 docs/specification 重写
- android/benchmark 需要更充分的性能测试并且断言测试结果，使用模拟器检查，优化提高性能
- termux.properties 的所有相关代码必须根据 docs/specification termux-app 重新检查和修改，并且每个键都需要充分测试
- 检查 docs/specification 的修改，对照项目进行修改 修复
