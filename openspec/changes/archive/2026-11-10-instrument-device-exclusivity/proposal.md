# Proposal

## Why

整轮 `connectedDebugAndroidTest` 偶发报「进程崩溃」，崩溃点每次不同（颜色像素验收、滚动
行为量化……），看上去像随机崩溃。根因不在被测代码：同一台设备上再起一轮仪器化时，框架
在安装测试包阶段以「设置调试应用」名义强杀正在跑的那一轮被测进程（API 35 及以上执行
`am set-debug-app`），被杀的一轮遂报「进程崩溃」。判别要靠 logcat 里同刻的强杀记录，
而这条判据此前只留在会话与审计留档里，没有进规约，于是同一现象被反复当成随机崩溃
重新排查。

## What Changes

- 规约 `build-and-validation` 新增一条要求：整轮仪器化 MUST 独占设备。
- 明确并发跑的判别口径：logcat 同刻出现「设置调试应用」的强杀记录即判为并发造成，
  MUST NOT 判为被测代码缺陷。
- 明确跑整轮前的检查义务：MUST 确认设备上没有别的仪器化轮次在跑。

## Capabilities

### Modified Capabilities

- `build-and-validation`：整轮仪器化的设备独占要求与崩溃归因口径。

## Impact

- `openspec/specs/build-and-validation/spec.md`
- 验证流程：跑整轮 `connectedDebugAndroidTest` 前先确认设备上没有别的仪器化轮次在跑。
