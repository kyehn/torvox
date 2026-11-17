任何情况下禁止对本文件进行任何修改

---

## 未被解决的问题

基于83d2025e40ce3b6adee48404934db583c8aa9817提交

（无：输入法弹出时切后台返回终端的闪烁已修复——遮罩改为运行期缓存、切后台只发布
缓存，设备在后台丢失也不影响遮罩；判据与用例见 `openspec/specs/render-stability`
的「运行期缓存离开时的画面作为开孔遮罩」「设备在后台丢失时仍发布缓存遮罩」两个
场景，以及 `BackgroundResumeFlickerInstrumentedTest` 的设备丢失用例）
